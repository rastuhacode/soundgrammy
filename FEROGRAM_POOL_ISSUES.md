# Ferogram: cancellation accounting and DC pool readiness

This is a handoff for work in the **ferogram repository**, not a SoundGrammy feature request. The observations below concern ferogram 0.6.5 and were also present in upstream commit [`7955d8d`](https://github.com/ankit-chaubey/ferogram/commit/7955d8d28029273930886b6f64ab68c0d1018cd8) when inspected. Recheck the current branch before changing it; either issue may have changed since then.

## Required workflow

Use test-driven development for **each** issue. First add a focused regression test against the unmodified ferogram behavior, run it, and record its expected failure. Only after the test demonstrates the defect should you change production behavior. Then run the new tests and the relevant existing suite. Keep the two issues distinguishable in tests and review, even if their fixes interact.

Do not treat the application report below as proof of the entire causal chain. Establish the library-level behavior with deterministic tests, and document any part that remains an inference.

## Issue 1 — cancelled RPCs leave a slot permanently counted as busy

In [`ferogram-mtsender/src/pool.rs`](https://github.com/ankit-chaubey/ferogram/blob/7955d8d28029273930886b6f64ab68c0d1018cd8/ferogram-mtsender/src/pool.rs#L291-L316), `send_via_slot` increments a slot's `in_flight` count before waiting for the RPC result and decrements it only after that wait completes. Dropping or aborting the future while it waits skips the decrement. The pool may subsequently believe an idle connection is busy, distort least-loaded-slot selection, and open unnecessary additional slots.

This is a general async-cancellation bug, not specific to audio or one application. In a local copy of ferogram 0.6.5, a test that aborted a pending call observed `in_flight == 1` on the original implementation; the count should return to zero when the call ends by cancellation, success, or error.

The first regression test should demonstrate that cancellation of a pending RPC restores accurate slot accounting. Include relevant completion/error paths so the correction cannot introduce a double decrement or underflow.

## Issue 2 — pool expansion can select a slot that has not completed required setup

The pool can create an extra slot when existing slots appear busy, up to its per-DC limit. In the [`get_or_create_slot` expansion path](https://github.com/ankit-chaubey/ferogram/blob/7955d8d28029273930886b6f64ab68c0d1018cd8/ferogram-mtsender/src/pool.rs#L227-L270), it opens a connection and makes the new slot available for the requested RPC. The [`invoke_on_dc` path](https://github.com/ankit-chaubey/ferogram/blob/7955d8d28029273930886b6f64ab68c0d1018cd8/ferogram-mtsender/src/pool.rs#L329-L336) requests a raw connection in this case. That path does not perform the client-level initialization and, where applicable, authorization transfer performed for the first connection. Meanwhile, [`ensure_dc_ready`](https://github.com/ankit-chaubey/ferogram/blob/7955d8d28029273930886b6f64ab68c0d1018cd8/ferogram/src/client/mod.rs#L4289-L4325) checks readiness per DC; an initialized first slot can therefore mask the state of a later slot. A new slot is not evicted merely because an RPC returns `CONNECTION_NOT_INITED`.

The expected invariant is broader than any particular implementation: a slot must not carry a normal RPC until its own auth key/session/DC state meets Telegram's requirements. Check the rules for reused versus new auth keys, foreign-DC authorization, media sessions, home-DC parallel-session limits, and PFS rather than assuming all slots share the first slot's state. Telegram documents [`initConnection` and layer negotiation](https://core.telegram.org/api/invoking), [DC authorization and parallel sessions](https://core.telegram.org/api/datacenter), and [the value of multiple media connections](https://core.telegram.org/api/optimisation).

The second regression test should force the pool-expansion condition and prove that a newly selected slot cannot send an ordinary RPC before its required setup has succeeded. Exercise at least the relevant foreign/media-DC route, and ensure a setup failure never leaves a selectable, apparently ready slot. A deterministic fake transport/server or equivalent test seam is preferable to a test that depends on live Telegram availability.

## External symptom and limits of evidence

SoundGrammy, a ferogram consumer, repeatedly cancelled and restarted `upload.getFile` work during rapid seeking. Its macOS logs showed `RPC 400: CONNECTION_NOT_INITED` for requested file ranges. This is a valid example of why cancellation and media-pool expansion matter, but the application itself need not be understood or changed for this task. The cancellation-count leak is directly reproduced by a unit test. The uninitialized-extra-slot explanation follows from the inspected source and matches the server error; it still needs its own failing ferogram regression test before a behavioral fix.

A short live seek test stopped reproducing the error after a local mitigation, but that is supporting evidence, not a substitute for deterministic upstream tests or a throughput check. In particular, globally restricting every DC to one connection may avoid this route while making download-heavy ferogram users worse off; Telegram explicitly supports and recommends multiple connections for file transfers.

## Completion criteria

- Each regression test is shown failing before its corresponding behavior change and passing afterward.
- Cancellation cannot leave an incorrect `in_flight` count.
- No RPC is routed through a newly created slot before the setup required for that slot is complete; failed or cancelled setup does not make it usable.
- Existing legitimate multi-connection media behavior is preserved, or any throughput trade-off is measured and explained.
- Relevant tests cover normal completion, errors, cancellation, and connection/setup failure, and the existing suite remains green.
