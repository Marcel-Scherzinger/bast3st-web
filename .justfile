# revert and reapply all SQLx migrations
remigrate:
    sqlx migrate revert --target-version 0
    sqlx migrate run

new-migration NAME:
    sqlx migrate add -r {{NAME}}

