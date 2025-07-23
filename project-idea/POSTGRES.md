orkarfabianthewise@FABIAN-THEWISE:~/code$ sudo service postgresql status
● postgresql.service - PostgreSQL RDBMS
Loaded: loaded (/lib/systemd/system/postgresql.service; enabled; vendor preset: enabled)
Active: active (exited) since Fri 2025-07-11 20:53:02 WAT; 25min ago
Process: 26649 ExecStart=/bin/true (code=exited, status=0/SUCCESS)
Main PID: 26649 (code=exited, status=0/SUCCESS)
CPU: 1ms

Jul 11 20:53:02 FABIAN-THEWISE systemd[1]: Starting PostgreSQL RDBMS...
Jul 11 20:53:02 FABIAN-THEWISE systemd[1]: Finished PostgreSQL RDBMS.
orkarfabianthewise@FABIAN-THEWISE:~/code$ sudo passwd postgres
New password:
Retype new password:
passwd: password updated successfully
orkarfabianthewise@FABIAN-THEWISE:~/code$ sudo -u postgres psql
could not change directory to "/home/orkarfabianthewise/code": Permission denied
psql (14.18 (Ubuntu 14.18-0ubuntu0.22.04.1))
Type "help" for help.

postgres=# help
You are using psql, the command-line interface to PostgreSQL.
Type: \copyright for distribution terms
\h for help with SQL commands
\? for help with psql commands
\g or terminate with semicolon to execute query
\q to quit

sudo -u postgres psql
could not change directory to "/home/orkarfabianthewise/code": Permission denied
psql (14.18 (Ubuntu 14.18-0ubuntu0.22.04.1))
Type "help" for help.

postgres=# CREATE ROLE orkarfabianthewise WITH
LOGIN SUPERUSER CREATEDB PASSWORD '2000';
CREATE ROLE
postgres=# \du
List of roles
Role name | Attributes | Member of
--------------------+------------------------------------------------------------+-----------
orkarfabianthewise | Superuser, Create DB | {}
postgres | Superuser, Create role, Create DB, Replication, Bypass RLS | {}

postgres=# \Q
invalid command \Q
Try \? for help.
postgres=# \q
orkarfabianthewise@FABIAN-THEWISE:~/code$ psql -U orkarfabianthewise -d postgres
