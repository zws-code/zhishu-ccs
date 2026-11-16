// A recovery may change publisher code, but must never change the signed build.
export async function resolveReleaseSource(github, repo, runId) {
  if (!/^[1-9]\d*$/.test(String(runId)))
    throw new Error("Provide the original formal release run ID");
  const id = Number(runId);
  if (!Number.isSafeInteger(id)) throw new Error("Invalid run ID");
  const { data: run } = await github.rest.actions.getWorkflowRun({
    ...repo,
    run_id: id,
  });
  const { data: workflow } = await github.rest.actions.getWorkflow({
    ...repo,
    workflow_id: run.workflow_id,
  });
  const tag = run.head_branch;
  if (
    workflow.path !== ".github/workflows/release.yml" ||
    run.event !== "push" ||
    run.status !== "completed" ||
    !["failure", "success", "cancelled"].includes(run.conclusion) ||
    run.head_repository?.full_name !== `${repo.owner}/${repo.repo}` ||
    !/^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(tag || "")
  )
    throw new Error(
      "Recovery requires a completed formal tag release from this repository",
    );
  if (run.conclusion === "cancelled") {
    const jobs = await github.paginate(
      github.rest.actions.listJobsForWorkflowRun,
      { ...repo, run_id: id, filter: "latest", per_page: 100 },
    );
    const publishers = jobs.filter(
      (job) => job.name === "Validate artifacts and publish to COS",
    );
    if (
      publishers.length !== 1 ||
      !publishers[0].steps?.some(
        (step) =>
          step.name === "Save verified payload for retries" &&
          step.conclusion === "success",
      )
    )
      throw new Error(
        "Cancelled release has no completed verified payload save",
      );
  }
  const { data: ref } = await github.rest.git.getRef({
    ...repo,
    ref: `tags/${tag}`,
  });
  let object = ref.object;
  for (let depth = 0; object.type === "tag" && depth < 5; depth++) {
    const { data } = await github.rest.git.getTag({
      ...repo,
      tag_sha: object.sha,
    });
    object = data.object;
  }
  if (object.type !== "commit" || object.sha !== run.head_sha)
    throw new Error("The original release tag has moved; refusing recovery");
  const artifacts = await github.paginate(
    github.rest.actions.listWorkflowRunArtifacts,
    { ...repo, run_id: id, per_page: 100 },
  );
  const name = `cos-payload-${tag}`;
  const matches = artifacts.filter(
    (item) => item.name === name && !item.expired,
  );
  if (matches.length !== 1)
    throw new Error(
      "The verified original COS payload is missing, expired or ambiguous",
    );
  return { tag, runId: String(id), sha: run.head_sha, artifact: name };
}
