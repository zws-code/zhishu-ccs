import assert from "node:assert/strict";
import { test } from "node:test";
import { resolveReleaseSource } from "./resume-release.mjs";

const repo = { owner: "zws-code", repo: "zhishu-ccs" };
function fixture(change = {}) {
  const run = {
    workflow_id: 1,
    event: "push",
    status: "completed",
    conclusion: "failure",
    head_branch: "v0.0.1",
    head_sha: "original-signed-build",
    head_repository: { full_name: "zws-code/zhishu-ccs" },
    ...change.run,
  };
  return {
    rest: {
      actions: {
        getWorkflowRun: async () => ({ data: run }),
        getWorkflow: async () => ({
          data: { path: change.path || ".github/workflows/release.yml" },
        }),
        listWorkflowRunArtifacts: () => {},
        listJobsForWorkflowRun: () => {},
      },
      git: {
        getRef: async () => ({
          data: { object: change.ref || { type: "commit", sha: run.head_sha } },
        }),
        getTag: async () => ({
          data: { object: { type: "commit", sha: run.head_sha } },
        }),
      },
    },
    paginate: async (method) =>
      method.name === "listJobsForWorkflowRun"
        ? change.jobs || []
        : change.artifacts || [{ name: "cos-payload-v0.0.1", expired: false }],
  };
}

test("recovery reuses only the original signed payload and unchanged release tag", async () => {
  const result = await resolveReleaseSource(fixture(), repo, "40825474319");
  assert.deepEqual(result, {
    tag: "v0.0.1",
    runId: "40825474319",
    sha: "original-signed-build",
    artifact: "cos-payload-v0.0.1",
  });
  await resolveReleaseSource(
    fixture({ ref: { type: "tag", sha: "annotated-tag" } }),
    repo,
    "40825474319",
  );
});

test("validation runs, other repositories/workflows and moved tags cannot publish", async () => {
  for (const change of [
    { run: { event: "workflow_dispatch" } },
    { run: { head_branch: "codex/cos-validation" } },
    { run: { status: "in_progress" } },
    { run: { conclusion: "cancelled" } },
    { run: { head_repository: { full_name: "other/fork" } } },
    { path: ".github/workflows/ci.yml" },
    { ref: { type: "commit", sha: "different-build" } },
    { artifacts: [] },
    { artifacts: [{ name: "cos-payload-v0.0.1", expired: true }] },
    {
      artifacts: Array(2).fill({ name: "cos-payload-v0.0.1", expired: false }),
    },
  ])
    await assert.rejects(resolveReleaseSource(fixture(change), repo, "123"));
  for (const runId of ["", "123; command", "-1", "9007199254740992"])
    await assert.rejects(resolveReleaseSource(fixture(), repo, runId));
});

test("cancelled formal releases require a completed verified payload save", async () => {
  const publisher = {
    name: "Validate artifacts and publish to COS",
    steps: [
      { name: "Save verified payload for retries", conclusion: "success" },
    ],
  };
  const cancelled = { run: { conclusion: "cancelled" }, jobs: [publisher] };
  const result = await resolveReleaseSource(fixture(cancelled), repo, "123");
  assert.equal(result.sha, "original-signed-build");
  for (const jobs of [
    [],
    [publisher, publisher],
    [{ ...publisher, steps: [] }],
    [
      {
        ...publisher,
        steps: [{ ...publisher.steps[0], conclusion: "cancelled" }],
      },
    ],
    [
      {
        ...publisher,
        steps: [{ ...publisher.steps[0], conclusion: "failure" }],
      },
    ],
  ])
    await assert.rejects(
      resolveReleaseSource(fixture({ ...cancelled, jobs }), repo, "123"),
      /completed verified payload save/,
    );
  for (const change of [
    { run: { conclusion: "cancelled", status: "in_progress" } },
    { ref: { type: "commit", sha: "moved-tag" } },
    { artifacts: [] },
    { artifacts: [{ name: "cos-payload-v0.0.1", expired: true }] },
  ])
    await assert.rejects(
      resolveReleaseSource(fixture({ ...cancelled, ...change }), repo, "123"),
    );
});
