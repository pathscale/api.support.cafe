# WorkTable v3 deployment cutover

This branch adopts WorkTable 1.9.0-alpha1 and DataBucket 0.7. The physical
page format changes from v2 to v3 even when an application row schema is
unchanged. The new reader refuses old files; changing a schema version or
renaming a file does not convert its contents.

The intended rollout for regenerable data is explicit recreation. Stop writers,
retain the old binary and a snapshot of its local directory and remote object
prefix, then configure a new empty local data directory and a new empty remote
prefix where S3 is enabled. Start the new build against that empty destination,
run the service bootstrap or regeneration procedure, verify the expected state,
shut it down cleanly and verify a cold reopen before routing traffic to it.

Do not delete the previous prefix or reuse it for v3 writes. Rollback selects
the old binary together with the old directory and prefix. New writes after
cutover are not automatically copied back into the old store.

An application that must retain records needs a schema-specific export using
the old reader and an import using the new writer. The migration crate in this
branch, if present, uses the new WorkTable dependency and is not a v2 reader.
Do not use it as proof of old-format compatibility. Existing beta-upgrade notes
and rebuild instructions do not override this v3 cutover policy.

Release order: publish the reviewed core dependencies, then WorkTable and its
matching codegen, then honey_id-types 2.1.0-alpha1 for consumers that use it,
then this application. Keep endpoint-libs at the matching 2.0.0 API generation.
Local PR checks use explicit temporary path patches for unpublished releases;
the committed manifests use registry requirements. Registry-only builds must
be repeated after publication.

Code review and unit tests do not authorize a live data reset or establish
production-data validation. The selected destination and bootstrap outcome
must be recorded by the deployment operator. No production data was changed
as part of this PR review.
