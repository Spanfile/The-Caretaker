CREATE TABLE "module_settings" (
    "guild" BIGINT NOT NULL,
    "module" module_kind NOT NULL,
    "setting" TEXT NOT NULL,
    "value" TEXT NOT NULL,
    PRIMARY KEY ("guild", "module", "setting")
);
