CREATE TABLE "enabled_modules" (
	"guild" BIGINT NOT NULL,
	"module" TEXT NOT NULL,
	"enabled" BOOLEAN NOT NULL,
	PRIMARY KEY("guild", "module")
);
