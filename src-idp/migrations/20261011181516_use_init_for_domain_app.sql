-- Add migration script here
DELETE FROM applications
WHERE slug = 'domain';
