# User-managed PostgreSQL

## Status

Accepted

## Decision

Nook releases require a user-managed PostgreSQL service. Installers must not
bundle, install, or manage PostgreSQL. They must preserve the configured
`database_url` and report an unreachable database before claiming Nook is ready.

## Context

The Worker stores conversation memory in PostgreSQL. Managing a local database
would add data-directory ownership, service lifecycle, backup, migration, and
upgrade responsibilities to each platform installer. This release keeps the
database outside the application package.

## Consequences

- Users must install and start PostgreSQL separately.
- Install and startup checks must use the configured connection URL.
- Package tests must check the missing-database failure path.
- Nook upgrades must not delete or rewrite user database files.
