//! Tests for login, logout, account switching, and auth-code dispatchers.

use super::*;

#[test]
fn cta_mcps_loaded_needs_auth_opens_modal_and_seeds() {
    use crate::app::agent_view::CtaPhase;
    use crate::views::extensions_modal::{ExtensionsTab, TabDataState};
    use crate::views::mcps_modal::{McpSectionId, McpServerDisplayStatus, section_key};
    let mut app = test_app_with_agent();
    app.team_id = Some("team-uuid".into());
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().plugin_cta.phase = CtaPhase::AwaitingMcps {
        name: "figma".into(),
    };
    let servers = vec![
        cta_mcp_server("managed-srv", None, McpServerDisplayStatus::Ready),
        cta_mcp_server("local-srv", None, McpServerDisplayStatus::Ready),
        cta_mcp_server("other-srv", Some("slack"), McpServerDisplayStatus::Ready),
        cta_mcp_server(
            "figma-srv",
            Some("figma"),
            McpServerDisplayStatus::NeedsAuth,
        ),
    ];
    let effects = dispatch(
        Action::TaskComplete(TaskResult::PluginCtaMcpsLoaded {
            agent_id: id,
            plugin_name: "figma".into(),
            result: Ok(servers),
        }),
        &mut app,
    );
    // The CTA is finished; the modal owns the flow from here
    assert_eq!(test_agent(&app, id).plugin_cta.phase, CtaPhase::Hidden);
    // Modal opened to the MCP Servers tab.
    let modal = test_agent(&app, id)
        .extensions_modal
        .as_ref()
        .expect("extensions modal should be open");
    assert_eq!(modal.active_tab, ExtensionsTab::McpServers);
    // Session team id seeded so the Managed subtitle deep link matches Ctrl+O.
    assert_eq!(modal.session_team_id.as_deref(), Some("team-uuid"));
    // MCP tab seeded directly from the read we already have (no flash).
    match &modal.mcps_data {
        TabDataState::Loaded(servers) => assert_eq!(servers.len(), 4),
        other => panic!("expected mcps_data Loaded, got {other:?}"),
    }
    // Managed, Local, and other plugins collapsed; only the target plugin expanded
    let collapsed = &modal.mcps_collapsed_sections;
    assert!(collapsed.contains(&section_key(&McpSectionId::Managed)));
    assert!(collapsed.contains(&section_key(&McpSectionId::Local)));
    assert!(collapsed.contains(&section_key(&McpSectionId::Plugin("slack".into()))));
    assert!(!collapsed.contains(&section_key(&McpSectionId::Plugin("figma".into()))));
    assert!(modal.mcps_section_collapse_initialized);
    // Emits the same full set of tab fetches as a manual open so no tab is stuck Loading, plus the candidate refresh
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::FetchHooksList { .. }))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::FetchPluginsList { .. }))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::FetchMarketplaceList { .. }))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::FetchMcpsList { .. }))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::FetchSkillsList { .. }))
            .count(),
        1
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::FetchPluginCtaCatalog { .. }))
    );
}

#[test]
fn cta_mcps_loaded_no_needs_auth_terminal_sets_installed() {
    use crate::app::agent_view::CtaPhase;
    use crate::views::mcps_modal::McpServerDisplayStatus;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let cta = &mut app.agents.get_mut(&id).unwrap().plugin_cta;
        cta.phase = CtaPhase::AwaitingMcps {
            name: "figma".into(),
        };
        cta.expects_mcp = true;
    }
    // The plugin server is present and Ready (terminal, no auth), so the CTA settles now
    let servers = vec![cta_mcp_server(
        "figma-srv",
        Some("figma"),
        McpServerDisplayStatus::Ready,
    )];
    let effects = dispatch(
        Action::TaskComplete(TaskResult::PluginCtaMcpsLoaded {
            agent_id: id,
            plugin_name: "figma".into(),
            result: Ok(servers),
        }),
        &mut app,
    );
    assert_eq!(
        test_agent(&app, id).plugin_cta.phase,
        CtaPhase::Installed {
            name: "figma".into()
        }
    );
    assert!(test_agent(&app, id).extensions_modal.is_none());
    // No modal repopulation; settling emits the auto-dismiss timer and the candidate refresh, and never re-fetches the MCP list
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::FetchMcpsList { .. }))
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::RetryPluginCtaMcps { .. }))
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::DismissCtaInstalled { .. }))
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::FetchPluginCtaCatalog { .. }))
    );
}

#[test]
fn cta_mcps_loaded_later_needs_auth_opens_handoff() {
    use crate::app::agent_view::CtaPhase;
    use crate::views::mcps_modal::McpServerDisplayStatus;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let cta = &mut app.agents.get_mut(&id).unwrap().plugin_cta;
        cta.phase = CtaPhase::AwaitingMcps {
            name: "figma".into(),
        };
        cta.expects_mcp = true;
        // Several polls already elapsed before the server reached NeedsAuth.
        cta.mcp_attempt = 5;
    }
    let effects = dispatch(
        Action::TaskComplete(TaskResult::PluginCtaMcpsLoaded {
            agent_id: id,
            plugin_name: "figma".into(),
            result: Ok(vec![cta_mcp_server(
                "figma-srv",
                Some("figma"),
                McpServerDisplayStatus::NeedsAuth,
            )]),
        }),
        &mut app,
    );
    // NeedsAuth is terminal: the modal opens immediately even mid-poll
    assert_eq!(test_agent(&app, id).plugin_cta.phase, CtaPhase::Hidden);
    assert!(test_agent(&app, id).extensions_modal.is_some());
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::RetryPluginCtaMcps { .. }))
    );
}

/// A bash command typed while a turn is running goes straight to the server: an Effect and an optimistic echo, no local queue entry.
#[test]
fn bash_while_running_is_server_authoritative() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().session.state = AgentState::TurnRunning;

    let effects = dispatch(Action::SendBashCommand("ls -la".into()), &mut app);
    let Some(effect) = effects.first() else {
        panic!("expected an effect, got {effects:?}");
    };
    let pid = match effect {
        Effect::SendBashCommand {
            command, prompt_id, ..
        } => {
            assert_eq!(command, "ls -la");
            prompt_id.clone()
        }
        other => panic!("expected immediate SendBashCommand, got {other:?}"),
    };
    assert_eq!(test_agent(&app, id).session.queue_len(), 0);
    // Optimistic echo present with kind="bash".
    let q = app
        .shared_prompt_queue("test-session")
        .expect("echo present");
    assert_eq!(q.len(), 1);
    let Some(front) = q.first() else {
        panic!("expected queue echo: {q:?}");
    };
    assert_eq!(front.id, pid);
    assert_eq!(front.kind, "bash");
    assert_eq!(front.text, "ls -la");
}

#[test]
fn auth_complete_triggers_bundle_status_fetch() {
    let mut app = test_app();
    app.auth_state = AuthState::Authenticating {
        request_seq: 1,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };

    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 1,
            meta: None,
        }),
        &mut app,
    );

    assert!(matches!(app.auth_state, AuthState::Done));
    // The pager only refreshes the on-disk catalog snapshot; the bundle download runs inside the shell after auth
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::FetchBundleStatus))
    );
}

#[test]
fn auth_complete_with_deferred_load_also_fetches_status() {
    let mut app = test_app();
    app.auth_state = AuthState::Authenticating {
        request_seq: 1,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    app.deferred_startup.session =
        Some(crate::app::session_startup::DeferredSessionStartup::Load {
            session_id: "test-session".into(),
            session_cwd: None,
            chat_kind: false,
        });

    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 1,
            meta: None,
        }),
        &mut app,
    );

    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::FetchBundleStatus))
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::LoadSession { .. }))
    );
    assert!(app.deferred_startup.session.is_none());
}

/// `/login` from the welcome screen (startup, logged out) must not stash a return view; the normal login-then-load flow is preserved.
#[test]
fn login_from_welcome_does_not_stash_return_view() {
    let mut app = test_app();
    assert_eq!(app.active_view, ActiveView::Welcome);

    dispatch(Action::Login, &mut app);

    assert_eq!(app.active_view, ActiveView::Welcome);
    assert_eq!(app.auth_return_view, None);
}

/// A build without account logins refuses a login, a logout or an account switch, in the session.
#[test]
fn without_account_logins_login_and_logout_are_refused_in_the_session() {
    let refusal =
        chutes_config::Distribution::withholding(&[chutes_config::Capability::AccountLogin])
            .refusal(chutes_config::Capability::AccountLogin)
            .expect("a build without account logins refuses one");
    for action in [Action::Login, Action::Logout, Action::SwitchAccount] {
        let mut app = test_app_with_agent();
        app.distribution =
            chutes_config::Distribution::withholding(&[chutes_config::Capability::AccountLogin]);
        let view = app.active_view;

        let effects = dispatch(action, &mut app);

        assert!(effects.is_empty(), "{effects:?}");
        assert_eq!(view, app.active_view, "no login screen");
        assert_eq!(refusal, last_system_text(&app, AgentId(0)));
    }
}

#[test]
fn without_account_logins_the_welcome_login_is_refused() {
    let mut app = test_app();
    app.distribution =
        chutes_config::Distribution::withholding(&[chutes_config::Capability::AccountLogin]);
    app.auth_state = AuthState::Pending { error: None };

    assert!(dispatch(Action::Login, &mut app).is_empty());
    assert!(matches!(app.auth_state, AuthState::Pending { .. }));
    let refusal =
        chutes_config::Distribution::withholding(&[chutes_config::Capability::AccountLogin])
            .refusal(chutes_config::Capability::AccountLogin);
    assert_eq!(
        refusal,
        app.welcome_toast
            .as_ref()
            .map(|(message, _)| message.as_str())
    );
}

/// Compact-auth recovery: the prompt is held across an auto-compact 401, stashed on PromptResponse, and resubmitted on a mid-session AuthComplete.
#[test]
fn e2e_compact_auth_failure_holds_prompt_and_resubmits_after_login() {
    use crate::app::acp_handler::apply_session_event_for_test;
    use crate::app::agent::{AgentState, InFlightPrompt};
    use crate::scrollback::EntryId;
    use crate::scrollback::block::RenderBlock;
    use chutes_shell::extensions::notification::{
        RetryState, SessionUpdate as ChutesSessionUpdate,
    };

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.turn_started_at = Some(std::time::Instant::now());
        agent.session.session_id = Some(acp::SessionId::new("sess-compact-auth-e2e"));
        agent.session.current_prompt_id = Some("prompt-1".into());
        agent.session.in_flight_prompt = Some(InFlightPrompt {
            text: "please continue after login".into(),
            images: Vec::new(),
            scrollback_entry: EntryId::new(1),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });

        apply_session_event_for_test(
            &ChutesSessionUpdate::AutoCompactStarted {
                tokens_used: 180_000,
                context_window: 200_000,
                percentage: 90,
                reason: "threshold".into(),
            },
            &mut agent.session,
            &mut agent.scrollback,
        );
        assert!(
            agent.session.in_flight_prompt.is_none(),
            "cancel rewind must still be blocked mid-compact"
        );
        assert_eq!(
            agent
                .session
                .compact_held_prompt
                .as_ref()
                .map(|p| p.text.as_str()),
            Some("please continue after login"),
            "must hold the prompt text for reauth auto-resubmit"
        );

        apply_session_event_for_test(
            &ChutesSessionUpdate::AutoCompactFailed {
                error: "authentication problem — re-authenticate using /login and retry.".into(),
            },
            &mut agent.session,
            &mut agent.scrollback,
        );
        assert!(agent.session.compact_held_prompt.is_some());

        apply_session_event_for_test(
            &ChutesSessionUpdate::RetryState(RetryState::Failed {
                error_type: "auth".into(),
                message: "Unauthorized (401): compaction failed".into(),
            }),
            &mut agent.session,
            &mut agent.scrollback,
        );
        let has_reauth = (0..agent.scrollback.len()).any(|i| {
            matches!(
                agent.scrollback.entry(i).map(|e| &e.block),
                Some(RenderBlock::SessionEvent(ev))
                    if matches!(ev.event, SessionEvent::ReAuthRequired)
            )
        });
        assert!(has_reauth, "RetryState auth must show ReAuthRequired");
    }

    dispatch(
        Action::TaskComplete(TaskResult::PromptResponse {
            agent_id: id,
            result: Err("Unauthorized (401)".to_string()),
            http_status: Some(401),
            prompt_id: Some("prompt-1".into()),
        }),
        &mut app,
    );
    assert_eq!(
        test_agent(&app, id)
            .reauth_stashed_prompt
            .as_ref()
            .map(|p| p.text.as_str()),
        Some("please continue after login"),
        "PromptResponse must stash the compact-held prompt for AuthComplete"
    );

    dispatch(Action::Login, &mut app);
    let seq = authenticating_seq(&app);
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: seq,
            meta: None,
        }),
        &mut app,
    );
    assert!(
        test_agent(&app, id).reauth_stashed_prompt.is_none(),
        "stash consumed on AuthComplete"
    );
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. }
        )),
        "AuthComplete must resubmit the prompt so compact runs again with valid auth, got: {effects:?}"
    );
}

/// Without `compact_held_prompt`, clearing `in_flight_prompt` when compact starts leaves nothing for reauth to stash.
#[test]
fn pre_fix_compact_start_without_hold_cannot_stash_for_reauth() {
    use crate::app::agent::AgentState;
    use crate::scrollback::block::RenderBlock;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.turn_started_at = Some(std::time::Instant::now());
        agent.session.session_id = Some(acp::SessionId::new("sess-pre-fix"));
        agent.session.current_prompt_id = Some("p1".into());
        agent.session.in_flight_prompt = None;
        agent.session.compact_held_prompt = None;
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::ReAuthRequired));
    }
    dispatch(
        Action::TaskComplete(TaskResult::PromptResponse {
            agent_id: id,
            result: Err("Unauthorized (401)".to_string()),
            http_status: Some(401),
            prompt_id: Some("p1".into()),
        }),
        &mut app,
    );
    assert!(
        test_agent(&app, id).reauth_stashed_prompt.is_none(),
        "without compact_held / in_flight, reauth cannot stash — the pre-fix bug"
    );
}

/// A second auth-failed turn with no rewindable prompt (`in_flight_prompt == None`) must not clobber the stash from an earlier 401.
#[test]
fn second_auth_failure_does_not_clobber_reauth_stash() {
    use crate::scrollback::block::RenderBlock;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.reauth_stashed_prompt = Some(crate::app::agent::InFlightPrompt {
            text: "first prompt".into(),
            images: Vec::new(),
            scrollback_entry: crate::scrollback::EntryId::new(0),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::ReAuthRequired));
        agent.session.state = AgentState::TurnRunning;
        agent.turn_started_at = Some(std::time::Instant::now());
        agent.session.in_flight_prompt = None;
    }

    dispatch(
        Action::TaskComplete(TaskResult::PromptResponse {
            agent_id: id,
            result: Err("Unauthorized (401)".to_string()),
            http_status: Some(401),
            prompt_id: None,
        }),
        &mut app,
    );

    assert_eq!(
        test_agent(&app, id)
            .reauth_stashed_prompt
            .as_ref()
            .map(|prompt| prompt.text.as_str()),
        Some("first prompt"),
        "a None in_flight_prompt must not wipe an earlier stash"
    );
}

/// Cancelling a mid-session re-auth drops the stashed prompt so it is not silently resubmitted on a later, unrelated login.
#[test]
fn cancel_login_drops_reauth_stashed_prompt() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().reauth_stashed_prompt =
        Some(crate::app::agent::InFlightPrompt {
            text: "stale".into(),
            images: Vec::new(),
            scrollback_entry: crate::scrollback::EntryId::new(0),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });

    dispatch(Action::Login, &mut app);
    dispatch(Action::CancelLogin, &mut app);

    assert!(
        test_agent(&app, id).reauth_stashed_prompt.is_none(),
        "cancelling re-auth must drop the stashed prompt"
    );
}

/// Cancelling a mid-session re-auth strips the stale `ReAuthRequired` prompt from scrollback.
/// A later `PromptResponse` can then no longer re-detect it and re-stash the prompt for silent resubmission.
#[test]
fn cancel_login_strips_reauth_prompt_from_scrollback() {
    use crate::scrollback::block::RenderBlock;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.reauth_stashed_prompt = Some(crate::app::agent::InFlightPrompt {
            text: "stale".into(),
            images: Vec::new(),
            scrollback_entry: crate::scrollback::EntryId::new(0),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::ReAuthRequired));
    }

    dispatch(Action::Login, &mut app);
    dispatch(Action::CancelLogin, &mut app);

    let sb = &test_agent(&app, id).scrollback;
    let has_reauth = (0..sb.len()).any(|i| {
        matches!(
            sb.entry(i).map(|e| &e.block),
            Some(RenderBlock::SessionEvent(ev)) if matches!(ev.event, SessionEvent::ReAuthRequired)
        )
    });
    assert!(
        !has_reauth,
        "cancelling re-auth must strip the stale re-auth prompt from scrollback"
    );
}

/// Empty `auth_methods` (the preferred_method pin is unavailable) must not invent `chutes.ai` or start an OIDC flow the agent did not advertise.
#[test]
fn login_with_empty_auth_methods_fails_closed() {
    let mut app = test_app_with_agent();
    app.auth_methods.clear();
    app.login_method_id = None;

    let effects = dispatch(Action::Login, &mut app);

    assert!(
        effects.is_empty(),
        "must not start Authenticate without an advertised method"
    );
    assert_eq!(
        app.active_view,
        ActiveView::Agent(AgentId(0)),
        "must stay on the session view"
    );
    assert!(
        matches!(
            &app.auth_state,
            AuthState::Pending { error: Some(msg) }
                if msg.contains("preferred_method=api_key")
        ),
        "must surface pin-unavailable error, got {:?}",
        app.auth_state
    );
    assert!(app.login_method_id.is_none());
}

/// Puts the app in `Authenticating` with a live task's abort handle installed, as the event loop would.
/// Returns the task's JoinHandle and the seq.
/// Callers assert the task actually gets aborted (`unwrap_err().is_cancelled()`), not merely that the handle slot was cleared.
fn install_live_auth_task(
    app: &mut AppView,
    rt: &tokio::runtime::Runtime,
) -> (tokio::task::JoinHandle<()>, u64) {
    dispatch(Action::Login, app);
    let task = rt.spawn(std::future::pending::<()>());
    match &mut app.auth_state {
        AuthState::Authenticating {
            handle,
            request_seq,
            ..
        } => {
            *handle = Some(task.abort_handle());
            (task, *request_seq)
        }
        other => panic!("expected Authenticating after Login, got {other:?}"),
    }
}

fn test_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

/// A second `/login` while already authenticating must abort the prior auth task and bump the seq.
/// Single-flight: never two device-code requests running at once.
#[test]
fn login_while_authenticating_aborts_prior_task() {
    let rt = test_runtime();
    let mut app = test_app_with_agent();
    let (prior_task, first_seq) = install_live_auth_task(&mut app, &rt);

    let effects = dispatch(Action::Login, &mut app);

    rt.block_on(async {
        assert!(
            prior_task.await.unwrap_err().is_cancelled(),
            "prior auth task must be aborted"
        );
    });
    match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => {
            assert!(
                *request_seq > first_seq,
                "re-login must bump request_seq for single-flight"
            );
        }
        other => panic!("expected Authenticating after re-Login, got {other:?}"),
    }
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Authenticate { .. })),
        "re-login must emit a new Authenticate"
    );
}

/// A stale `AuthComplete` (its abort lost the race because the task had already finished) must not complete the new attempt.
/// The request-seq guard is the only protection here.
#[test]
fn stale_auth_complete_after_relogin_is_ignored() {
    let mut app = test_app_with_agent();
    dispatch(Action::Login, &mut app);
    let first_seq = match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => *request_seq,
        other => panic!("expected Authenticating after Login, got {other:?}"),
    };
    dispatch(Action::Login, &mut app); // re-login bumps to seq2

    dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: first_seq,
            meta: None,
        }),
        &mut app,
    );

    match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => {
            assert!(
                *request_seq > first_seq,
                "stale AuthComplete must leave the new attempt authenticating"
            );
        }
        other => panic!("stale AuthComplete must be ignored, got {other:?}"),
    }
}

/// Switch-account while authenticating goes through the same single-flight abort as `/login` (sibling entry point).
#[test]
fn switch_account_while_authenticating_aborts_prior_task() {
    let rt = test_runtime();
    let mut app = test_app_with_agent();
    let (prior_task, first_seq) = install_live_auth_task(&mut app, &rt);

    dispatch(Action::SwitchAccount, &mut app);

    rt.block_on(async {
        assert!(
            prior_task.await.unwrap_err().is_cancelled(),
            "prior auth task must be aborted on switch-account"
        );
    });
    match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => {
            assert!(*request_seq > first_seq, "switch must bump request_seq");
        }
        other => panic!("expected Authenticating after SwitchAccount, got {other:?}"),
    }
}

/// Cancelling a mid-session login aborts the in-flight auth task (not just restores the view) so a retry cannot race a still-polling prior attempt.
#[test]
fn cancel_login_aborts_prior_task() {
    let rt = test_runtime();
    let mut app = test_app_with_agent();
    // Login from a session view stashes `auth_return_view`, making CancelLogin live.
    let (prior_task, _) = install_live_auth_task(&mut app, &rt);

    dispatch(Action::CancelLogin, &mut app);

    rt.block_on(async {
        assert!(
            prior_task.await.unwrap_err().is_cancelled(),
            "cancel must abort the in-flight auth task"
        );
    });
}

/// Cancelling a mid-session login returns to the session rather than quitting the app, and clears the stashed view and auth state.
#[test]
fn cancel_login_restores_view() {
    let mut app = test_app_with_agent();
    dispatch(Action::Login, &mut app);
    assert_eq!(app.active_view, ActiveView::Welcome);
    let prior_seq = match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => *request_seq,
        other => panic!("expected Authenticating after Login, got {other:?}"),
    };

    let effects = dispatch(Action::CancelLogin, &mut app);

    assert!(
        matches!(
            effects.as_slice(),
            [Effect::CancelAuth { request_seq }] if *request_seq == prior_seq
        ),
        "cancel must tell the shell to stop the in-flight auth poll for this attempt"
    );
    assert_eq!(app.active_view, ActiveView::Agent(AgentId(0)));
    assert_eq!(app.auth_return_view, None);
    assert!(matches!(app.auth_state, AuthState::Done));
}

/// `CancelLogin` outside a mid-session login is a no-op (must not move off the welcome screen or panic).
#[test]
fn cancel_login_noop_without_stashed_view() {
    let mut app = test_app();
    let effects = dispatch(Action::CancelLogin, &mut app);
    assert!(effects.is_empty());
    assert_eq!(app.active_view, ActiveView::Welcome);
    assert_eq!(app.auth_return_view, None);
}

#[test]
fn auth_complete_extracts_show_resolved_model_from_meta() {
    let mut app = test_app();
    app.auth_state = AuthState::Authenticating {
        request_seq: 1,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    assert!(app.show_resolved_model);

    dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 1,
            meta: Some(serde_json::json!({ "show_resolved_model": false })),
        }),
        &mut app,
    );

    assert!(!app.show_resolved_model);
}

#[test]
fn auth_complete_preserves_show_resolved_model_when_absent() {
    let mut app = test_app();
    app.show_resolved_model = false;
    app.auth_state = AuthState::Authenticating {
        request_seq: 1,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };

    dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 1,
            meta: Some(serde_json::to_value(chutes_login::AuthMeta::default()).unwrap()),
        }),
        &mut app,
    );

    assert!(!app.show_resolved_model);
}

fn pending_app() -> AppView {
    let mut app = test_app_with_agent();
    app.auth_state = AuthState::Pending { error: None };
    app
}

fn entry_seq(app: &AppView) -> u64 {
    match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => *request_seq,
        other => panic!("expected entry mode, got {other:?}"),
    }
}

#[test]
fn enter_api_key_opens_entry_mode_from_pending() {
    let mut app = pending_app();
    let effects = dispatch(Action::EnterApiKey, &mut app);
    assert!(effects.is_empty());
    assert!(matches!(
        app.auth_state,
        AuthState::Authenticating {
            mode: AuthMode::ApiKeyEntry,
            ..
        }
    ));
    assert!(app.auth_code_input.text().is_empty());
    assert_eq!(app.api_key_entry_error, None);
    assert!(!app.api_key_entry_validating);
}

#[test]
fn enter_api_key_ignores_non_pending_states() {
    let mut app = test_app_with_agent();
    assert!(matches!(app.auth_state, AuthState::Done));
    assert!(dispatch(Action::EnterApiKey, &mut app).is_empty());
    assert!(matches!(app.auth_state, AuthState::Done));
}

#[test]
fn submit_api_key_emits_validation_effect() {
    let mut app = pending_app();
    let _ = dispatch(Action::EnterApiKey, &mut app);
    let _ = app.auth_code_input.insert_paste("cpk-test-key");
    let effects = dispatch(
        Action::SubmitApiKey(crate::app::actions::RedactedSecret::new(
            "cpk-test-key".to_string(),
        )),
        &mut app,
    );
    assert!(
        matches!(
            effects.as_slice(),
            [Effect::ValidateApiKey { request_seq, key }]
                if *request_seq == entry_seq(&app) && key.expose() == "cpk-test-key"
        ),
        "unexpected effects: {effects:?}"
    );
    assert!(app.api_key_entry_validating);
}

#[test]
fn submit_api_key_ignores_blank_key_and_wrong_mode() {
    let mut app = pending_app();
    let _ = dispatch(Action::EnterApiKey, &mut app);
    let blank = dispatch(
        Action::SubmitApiKey(crate::app::actions::RedactedSecret::new("  ".to_string())),
        &mut app,
    );
    assert!(blank.is_empty());
    assert!(!app.api_key_entry_validating);

    let mut app = test_app_with_agent();
    let effects = dispatch(
        Action::SubmitApiKey(crate::app::actions::RedactedSecret::new(
            "cpk-test-key".to_string(),
        )),
        &mut app,
    );
    assert!(effects.is_empty());
}

#[test]
fn cancel_api_key_entry_returns_to_pending_and_bumps_seq() {
    let mut app = pending_app();
    let _ = dispatch(Action::EnterApiKey, &mut app);
    let seq = entry_seq(&app);
    let _ = app.auth_code_input.insert_paste("cpk-test-key");
    let effects = dispatch(Action::CancelApiKeyEntry, &mut app);
    assert!(effects.is_empty());
    assert!(matches!(app.auth_state, AuthState::Pending { error: None }));
    assert!(app.auth_code_input.text().is_empty());
    // A late validation result for the cancelled attempt must not apply.
    let effects = dispatch(
        Action::TaskComplete(TaskResult::ApiKeyAccepted { request_seq: seq }),
        &mut app,
    );
    assert!(effects.is_empty());
    assert!(matches!(app.auth_state, AuthState::Pending { .. }));
}

#[test]
fn redacted_secret_debug_hides_the_key() {
    let action = Action::SubmitApiKey(crate::app::actions::RedactedSecret::new(
        "cpk-super-secret".to_string(),
    ));
    let debug = format!("{action:?}");
    assert!(debug.contains("[REDACTED]"), "{debug}");
    assert!(!debug.contains("cpk-super-secret"), "{debug}");
}

#[serial_test::serial(CHUTES_BUILD_HOME)]
#[test]
fn api_key_accepted_saves_and_pushes_key_to_agent() {
    let home = tempfile::tempdir().expect("temp home");
    let _home_guard = chutes_test_support::env::EnvGuard::set("CHUTES_BUILD_HOME", home.path());
    let prior_key = std::env::var_os("CHUTES_API_KEY");
    // SAFETY: serialized via `serial_test`; restored below.
    unsafe { std::env::remove_var("CHUTES_API_KEY") };

    let mut app = pending_app();
    let _ = dispatch(Action::EnterApiKey, &mut app);
    let _ = app.auth_code_input.insert_paste("cpk-test-key");
    let seq = entry_seq(&app);
    let effects = dispatch(
        Action::TaskComplete(TaskResult::ApiKeyAccepted { request_seq: seq }),
        &mut app,
    );

    // The agent is a separate process: the key must travel over ACP before
    // the flow can finish, or session creation fails with "no auth method
    // id provided".
    assert_eq!(effects.len(), 1, "unexpected effects: {effects:?}");
    let AuthState::Authenticating {
        request_seq: auth_seq,
        mode: AuthMode::ApiKeyEntry,
        ..
    } = &app.auth_state
    else {
        panic!("accepted must stay in entry mode, got {:?}", app.auth_state);
    };
    assert_ne!(*auth_seq, seq, "handshake uses a fresh seq");
    assert!(
        matches!(
            &effects[0],
            Effect::AuthenticateWithApiKey { request_seq, key }
                if *request_seq == *auth_seq && key.expose() == "cpk-test-key"
        ),
        "unexpected effects: {effects:?}"
    );
    assert!(app.api_key_entry_validating, "spinner covers the handshake");

    assert!(app.is_api_key_auth);
    assert_eq!(app.auth_methods.len(), 1);
    assert_eq!(
        app.auth_methods[0].id().0.as_ref(),
        chutes_shell::agent::auth_method::CHUTES_API_KEY_METHOD_ID
    );
    assert!(!app.usage_visible, "startup-with-key hides billing");
    assert_eq!(
        chutes_build_core::api_key_store::load(home.path()).as_deref(),
        Some("cpk-test-key")
    );
    assert_eq!(
        std::env::var("CHUTES_API_KEY").as_deref(),
        Ok("cpk-test-key")
    );

    // SAFETY: serialized via `serial_test`; restore the env for other tests.
    unsafe {
        match prior_key {
            Some(value) => std::env::set_var("CHUTES_API_KEY", value),
            None => std::env::remove_var("CHUTES_API_KEY"),
        }
    }
}

#[serial_test::serial(CHUTES_BUILD_HOME)]
#[test]
fn api_key_entry_auth_complete_finishes_and_resets_entry_flags() {
    let home = tempfile::tempdir().expect("temp home");
    let _home_guard = chutes_test_support::env::EnvGuard::set("CHUTES_BUILD_HOME", home.path());
    let prior_key = std::env::var_os("CHUTES_API_KEY");
    // SAFETY: serialized via `serial_test`; restored below.
    unsafe { std::env::remove_var("CHUTES_API_KEY") };

    let mut app = pending_app();
    let _ = dispatch(Action::EnterApiKey, &mut app);
    let _ = app.auth_code_input.insert_paste("cpk-test-key");
    let seq = entry_seq(&app);
    let _ = dispatch(
        Action::TaskComplete(TaskResult::ApiKeyAccepted { request_seq: seq }),
        &mut app,
    );
    let AuthState::Authenticating { request_seq, .. } = &app.auth_state else {
        panic!(
            "accepted must stay authenticating, got {:?}",
            app.auth_state
        );
    };
    let auth_seq = *request_seq;

    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: auth_seq,
            meta: None,
        }),
        &mut app,
    );

    assert!(matches!(app.auth_state, AuthState::Done));
    assert!(!app.api_key_entry_validating);
    assert!(app.api_key_entry_error.is_none());
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::FetchBundleStatus)),
        "entry converges on the normal completion path, got: {effects:?}"
    );

    // SAFETY: serialized via `serial_test`; restore the env for other tests.
    unsafe {
        match prior_key {
            Some(value) => std::env::set_var("CHUTES_API_KEY", value),
            None => std::env::remove_var("CHUTES_API_KEY"),
        }
    }
}

#[serial_test::serial(CHUTES_BUILD_HOME)]
#[test]
fn api_key_entry_auth_failed_rolls_back_to_pending() {
    let home = tempfile::tempdir().expect("temp home");
    let _home_guard = chutes_test_support::env::EnvGuard::set("CHUTES_BUILD_HOME", home.path());
    let prior_key = std::env::var_os("CHUTES_API_KEY");
    // SAFETY: serialized via `serial_test`; restored below.
    unsafe { std::env::remove_var("CHUTES_API_KEY") };

    let mut app = pending_app();
    let _ = dispatch(Action::EnterApiKey, &mut app);
    let _ = app.auth_code_input.insert_paste("cpk-test-key");
    let seq = entry_seq(&app);
    let _ = dispatch(
        Action::TaskComplete(TaskResult::ApiKeyAccepted { request_seq: seq }),
        &mut app,
    );
    let AuthState::Authenticating { request_seq, .. } = &app.auth_state else {
        panic!(
            "accepted must stay authenticating, got {:?}",
            app.auth_state
        );
    };
    let auth_seq = *request_seq;

    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthFailed {
            request_seq: auth_seq,
            error: "agent unreachable".to_string(),
        }),
        &mut app,
    );

    assert!(effects.is_empty());
    assert!(matches!(
        app.auth_state,
        AuthState::Pending { error: Some(_) }
    ));
    assert!(!app.api_key_entry_validating);
    assert!(!app.is_api_key_auth, "optimistic key auth rolls back");
    assert!(app.auth_methods.is_empty());

    // SAFETY: serialized via `serial_test`; restore the env for other tests.
    unsafe {
        match prior_key {
            Some(value) => std::env::set_var("CHUTES_API_KEY", value),
            None => std::env::remove_var("CHUTES_API_KEY"),
        }
    }
}

#[test]
fn api_key_accepted_drops_stale_seq() {
    let mut app = pending_app();
    let _ = dispatch(Action::EnterApiKey, &mut app);
    let effects = dispatch(
        Action::TaskComplete(TaskResult::ApiKeyAccepted {
            request_seq: entry_seq(&app) + 100,
        }),
        &mut app,
    );
    assert!(effects.is_empty());
    assert!(matches!(app.auth_state, AuthState::Authenticating { .. }));
}

#[test]
fn api_key_rejected_shows_error_and_stays_in_entry_mode() {
    let mut app = pending_app();
    let _ = dispatch(Action::EnterApiKey, &mut app);
    let _ = app.auth_code_input.insert_paste("cpk-test-key");
    let _ = dispatch(
        Action::SubmitApiKey(crate::app::actions::RedactedSecret::new(
            "cpk-test-key".to_string(),
        )),
        &mut app,
    );
    assert!(app.api_key_entry_validating);
    let effects = dispatch(
        Action::TaskComplete(TaskResult::ApiKeyRejected {
            request_seq: entry_seq(&app),
            error: "Invalid API key: Chutes rejected it.".to_string(),
        }),
        &mut app,
    );
    assert!(effects.is_empty());
    assert!(!app.api_key_entry_validating);
    assert_eq!(
        app.api_key_entry_error.as_deref(),
        Some("Invalid API key: Chutes rejected it.")
    );
    assert!(matches!(
        app.auth_state,
        AuthState::Authenticating {
            mode: AuthMode::ApiKeyEntry,
            ..
        }
    ));
    // The typed key survives so the user can fix and resubmit it.
    assert_eq!(app.auth_code_input.text(), "cpk-test-key");
}

#[test]
fn api_key_rejected_drops_stale_seq() {
    let mut app = pending_app();
    let _ = dispatch(Action::EnterApiKey, &mut app);
    let effects = dispatch(
        Action::TaskComplete(TaskResult::ApiKeyRejected {
            request_seq: entry_seq(&app) + 100,
            error: "late".to_string(),
        }),
        &mut app,
    );
    assert!(effects.is_empty());
    assert_eq!(app.api_key_entry_error, None);
}
