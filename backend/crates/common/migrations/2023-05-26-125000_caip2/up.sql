ALTER TABLE
  networks
ADD
  COLUMN caip2 TEXT UNIQUE;

UPDATE
  networks
SET
  caip2 = 'eip155:1'
WHERE
  NAME = 'mainnet';
