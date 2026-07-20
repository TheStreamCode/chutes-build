//! Jujutsu extension handlers — delegates to [`xai_grok_workspace::session::jj`].

use agent_client_protocol as acp;

use super::{Empty, ExtResult, to_ext_response, to_ext_response_partial};
use xai_grok_workspace::session::git::{CommitData, StageData};
use xai_grok_workspace::session::jj;

/// Handle a `chutes.build/git/*` method for a jj-colocated repo.
///
/// Returns `Some(result)` if handled, `None` to fall through to git.
pub async fn try_handle(
    method: &str,
    git_root: &std::path::Path,
    raw_params: &serde_json::value::RawValue,
) -> Option<ExtResult> {
    match method {
        "chutes.build/git/status" => Some(to_ext_response(jj::status(git_root).await)),
        "chutes.build/git/info" => Some(to_ext_response(jj::info(git_root).await)),
        // git HEAD points at `@-` in a colocated repo; route to jj so we report
        // the working-copy commit (`@`), consistent with `status`/`info`.
        "chutes.build/git/current_commit" => {
            Some(to_ext_response(jj::current_commit(git_root).await))
        }
        "chutes.build/git/branches" => Some(to_ext_response(jj::list_bookmarks(git_root).await)),

        // jj has no staging area — stage/unstage are no-ops
        "chutes.build/git/stage" => Some(to_ext_response(Ok(StageData { paths: Vec::new() }))),
        "chutes.build/git/stage/content" | "chutes.build/git/unstage" => {
            Some(to_ext_response(Ok(Empty {})))
        }

        "chutes.build/git/discard" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Req {
                #[serde(default)]
                paths: Option<Vec<String>>,
            }
            let req: Req = serde_json::from_str(raw_params.get()).ok()?;
            Some(to_ext_response(
                jj::discard(git_root, req.paths).await.map(|_| Empty {}),
            ))
        }

        "chutes.build/git/commit" => {
            #[derive(serde::Deserialize)]
            struct Req {
                message: String,
            }
            let req: Req = serde_json::from_str(raw_params.get()).ok()?;
            let result = jj::commit(git_root, &req.message).await;
            Some(match result {
                Ok(r) => to_ext_response_partial(Ok(r.data), r.warning),
                Err(e) => to_ext_response(Err::<CommitData, _>(e)),
            })
        }

        // Operations that don't apply to jj
        "chutes.build/git/checkout" => Some(Err(acp::Error::invalid_params()
            .data("checkout is not supported in jj repos; use `jj new` or `jj edit`"))),
        "chutes.build/git/stash" => Some(Err(acp::Error::invalid_params()
            .data("stash is not supported in jj repos; changes are always committed"))),

        // Everything else (diffs, files, serialize_changes) falls through to git
        _ => None,
    }
}
