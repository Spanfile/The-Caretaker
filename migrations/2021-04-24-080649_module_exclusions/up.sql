CREATE TYPE exclusion_kind AS ENUM (
    'user',
    'role'
);

CREATE TABLE "module_exclusions" (
    "guild" BIGINT NOT NULL,
    "module" module_kind NOT NULL,
    "kind" exclusion_kind NOT NULL,
    "id" BIGINT NOT NULL,
    PRIMARY KEY ("guild", "module"),
    UNIQUE ("kind", "id")
);
