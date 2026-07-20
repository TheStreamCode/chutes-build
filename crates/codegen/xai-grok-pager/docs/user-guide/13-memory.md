# Memory

Memory is local and enabled by default. Chutes Build maintains project knowledge
in `memories.md` and uses the user-level memory store under `~/.chutes-build`.

Memory writes are filtered for known secret formats, but filtering is not a
substitute for keeping credentials out of prompts. Review `memories.md` before
committing it and add it to `.gitignore` when project memory should remain
private.

Start a stateless session with:

```powershell
chutes-build --no-memory
```

The agent should record durable project facts and decisions, not full chat
transcripts, temporary debugging output, credentials, or personal data.
