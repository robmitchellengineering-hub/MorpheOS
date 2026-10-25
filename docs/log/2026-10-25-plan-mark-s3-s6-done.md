# plan/mark-s3-s6-done — mark S3 and S6 done

S3 and S6 both satisfy their doc-20 "Done when" now that their VM scenarios are merged and green.

- S3: #131 (decision + fetch), #135 (autoupdate image + `good`), #141 (`bad` + `tampered`) —
  all three scenarios green in the `images` workflow.
- S6: #132 (flow-timeline crate), #136 (rewind VM scenario) — `test-rewind.sh` green in the
  `images` workflow.

Doc-20 status lines changed from `open` to `done` and each got an **Evidence:** line.
