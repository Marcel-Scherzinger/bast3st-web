CREATE TABLE users (
    userid SERIAL PRIMARY KEY,
    username VARCHAR(255) UNIQUE NOT NULL,
    mainpassword VARCHAR(255) NOT NULL,
    unconfirmed_pwd VARCHAR(255) NULL
);

CREATE TABLE slots (
    slotid SERIAL PRIMARY KEY,
    userid INTEGER NOT NULL REFERENCES users,
    slotname VARCHAR(255) NOT NULL,
    UNIQUE (userid, slotname)
);

CREATE TABLE specifications (
    specid SERIAL PRIMARY KEY,
    slotid INTEGER NOT NULL REFERENCES slots,
    -- the biggest generation number is the latest specification
    generation INTEGER NOT NULL,
    -- should the generation be considered (useful to deactivate latest without changing order)
    active BOOL NOT NULL DEFAULT true,
    -- the specification
    content JSON NOT NULL,
    UNIQUE (slotid, generation)
);

CREATE TABLE submissions (
    subid SERIAL PRIMARY KEY,
    -- the specification this submission was checked against
    specid INTEGER NOT NULL REFERENCES specifications,
    -- a timestamp when this submission was processed
    createdat TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    -- what kind of application/medium sent the submission
    agent VARCHAR(255),
    -- a session id or pseudonym for the one handing in the submission
    sessionid VARCHAR(255),
    -- the program that was checked
    program JSON NOT NULL,
    -- the report that was generated
    report JSON
);

CREATE TABLE debug_submissions (
    debid SERIAL PRIMARY KEY,
    -- the specification this submission was checked against
    spec JSON NOT NULL,
    -- a timestamp when this submission was processed
    createdat TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    -- what kind of application/medium sent the submission
    agent VARCHAR(255),
    -- a session id or pseudonym for the one handing in the submission
    sessionid VARCHAR(255),
    -- the program that was checked
    program JSON NOT NULL,
    -- the report that was generated
    report JSON
);
