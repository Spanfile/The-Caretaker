CREATE TABLE "enabled_modules" (
	"guild" INTEGER NOT NULL,
	"module" TEXT NOT NULL,
	"enabled" INTEGER NOT NULL,
	PRIMARY KEY("guild", "module")
);
