ALTER TABLE "module_exclusions" DROP CONSTRAINT module_exclusions_pkey;
ALTER TABLE "module_exclusions" DROP CONSTRAINT module_exclusions_kind_id_key;
ALTER TABLE "module_exclusions" ADD PRIMARY KEY ("guild", "module", "kind", "id");
