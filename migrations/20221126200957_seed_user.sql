-- Add migration script here
INSERT INTO users (user_id, username, password_hash) 
VALUES (
    'ddf8994f-d522-4659-8d02-c1d479057be6',
    'admin',
    '$argon2id$v=19$m=4096,t=3,p=1$nUxVkl/dRMr8ANzD4hej9g$VavaWtXCAzTWRZgdA9jVUHR8Welja+F3XZ9vM9enSJ0'
);