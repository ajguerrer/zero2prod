use std::time::Duration;

use sqlx::{query, query_as, PgPool, Postgres, Transaction};
use tracing::{error, field::display, Span};
use uuid::Uuid;

use crate::{
    configuration::Settings, domain::SubscriberEmail, email_client::EmailClient,
    startup::get_connection_pool,
};

type PgTransaction = Transaction<'static, Postgres>;

struct NewsletterIssue {
    title: String,
    text_content: String,
    html_content: String,
}

pub enum ExecutionOutcome {
    TaskCompleted,
    EmptyQueue,
}

const MAX_RETRIES: i16 = 3;

pub async fn run_worker_until_stopped(config: Settings) -> ! {
    let db_pool = get_connection_pool(&config.database);
    let email_client = config.email_client.client();

    tokio::select! {
        _ = execute_task_loop(&db_pool, &email_client) => {},
        _ = prune_idempotency_table_loop(&db_pool) => {},
    };

    unreachable!()
}

async fn execute_task_loop(db_pool: &PgPool, email_client: &EmailClient) -> ! {
    loop {
        match try_execute_task(db_pool, email_client).await {
            Ok(ExecutionOutcome::TaskCompleted) => {}
            Ok(ExecutionOutcome::EmptyQueue) => tokio::time::sleep(Duration::from_secs(10)).await,
            Err(_) => tokio::time::sleep(Duration::from_secs(1)).await,
        }
    }
}

async fn prune_idempotency_table_loop(db_pool: &PgPool) -> ! {
    loop {
        match prune_idempotency_table(db_pool).await {
            Ok(_) => {}
            Err(e) => error!(error = %e, "Failed to prune idempotency table."),
        }
        tokio::time::sleep(Duration::from_secs(1000)).await;
    }
}

#[tracing::instrument(skip_all, fields(newsletter_issue_id, subscriber_email))]
pub async fn try_execute_task(
    db_pool: &PgPool,
    email_client: &EmailClient,
) -> Result<ExecutionOutcome, sqlx::Error> {
    if let Some((transaction, issue_id, email, retries)) = dequeue_task(db_pool).await? {
        Span::current()
            .record("newsletter_issue_id", display(issue_id))
            .record("subscriber_email", display(&email));

        let email = match SubscriberEmail::parse(email.clone()) {
            Ok(email) => email,
            Err(e) => {
                error!(
                    error = %e,
                    "Subscriber contact details are invalid. Skipping."
                );
                delete_task(transaction, issue_id, &email).await?;
                return Ok(ExecutionOutcome::TaskCompleted);
            }
        };

        let issue = get_issue(db_pool, issue_id).await?;
        match email_client
            .send(
                &email,
                &issue.title,
                &issue.html_content,
                &issue.text_content,
            )
            .await
        {
            Err(e) => {
                error!(
                    error = %e,
                    "Failed to deliver issue to confirmed subscriber. Retries {retries}/{MAX_RETRIES}."
                );
                retry_or_delete_task(transaction, issue_id, email.as_ref(), retries).await?;
            }
            Ok(()) => {
                delete_task(transaction, issue_id, email.as_ref()).await?;
            }
        }
        Ok(ExecutionOutcome::TaskCompleted)
    } else {
        Ok(ExecutionOutcome::EmptyQueue)
    }
}

async fn retry_or_delete_task(
    transaction: PgTransaction,
    issue_id: Uuid,
    email: &str,
    retries: i16,
) -> Result<(), sqlx::Error> {
    if retries <= MAX_RETRIES {
        update_task_retries(transaction, issue_id, email).await
    } else {
        delete_task(transaction, issue_id, email).await
    }
}

#[tracing::instrument(skip_all)]
async fn dequeue_task(
    db_pool: &PgPool,
) -> Result<Option<(PgTransaction, Uuid, String, i16)>, sqlx::Error> {
    let mut transaction = db_pool.begin().await?;
    Ok(query!(
        r#"
    SELECT newsletter_issue_id, subscriber_email, n_retries
    FROM issue_delivery_queue
    WHERE execute_after <= now()
    FOR UPDATE
    SKIP LOCKED
    LIMIT 1
    "#
    )
    .fetch_optional(&mut transaction)
    .await?
    .map(|r| {
        (
            transaction,
            r.newsletter_issue_id,
            r.subscriber_email,
            r.n_retries,
        )
    }))
}

#[tracing::instrument(skip_all)]
async fn delete_task(
    mut transaction: PgTransaction,
    issue_id: Uuid,
    email: &str,
) -> Result<(), sqlx::Error> {
    query!(
        r#"
    DELETE FROM issue_delivery_queue
    WHERE
        newsletter_issue_id = $1 AND
        subscriber_email = $2
        "#,
        issue_id,
        email
    )
    .execute(&mut transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}

#[tracing::instrument(skip_all)]
async fn update_task_retries(
    mut transaction: PgTransaction,
    issue_id: Uuid,
    email: &str,
) -> Result<(), sqlx::Error> {
    query!(
        r#"
    UPDATE issue_delivery_queue
    SET n_retries = n_retries + 1,
        execute_after = now() + interval '1 second' * n_retries ^ 2
    WHERE
        newsletter_issue_id = $1 AND
        subscriber_email = $2
        "#,
        issue_id,
        email
    )
    .execute(&mut transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}

#[tracing::instrument(skip_all)]
async fn get_issue(db_pool: &PgPool, issue_id: Uuid) -> Result<NewsletterIssue, sqlx::Error> {
    query_as!(
        NewsletterIssue,
        r#"
    SELECT title, text_content, html_content
    FROM newsletter_issues
    WHERE
        newsletter_issue_id = $1
        "#,
        issue_id
    )
    .fetch_one(db_pool)
    .await
}

#[tracing::instrument(skip_all)]
pub async fn prune_idempotency_table(db_pool: &PgPool) -> Result<u64, sqlx::Error> {
    Ok(query!(
        r#"
    DELETE FROM idempotency
    WHERE (created_at + interval '1 day') <  now()"#
    )
    .execute(db_pool)
    .await?
    .rows_affected())
}
