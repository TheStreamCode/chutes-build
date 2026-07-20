# Background Tasks and Monitoring

Long-running commands and subagents can continue in the background while the
main conversation progresses. Each task returns an identifier used to inspect
output, wait for completion, or terminate that exact task.

Use background execution for servers, builds, independent research, and
parallel workers. Keep interactive programs in the foreground. Do not start
duplicate monitors when one task can observe the same condition.

Scheduled and recurring work must stay within the permissions granted for the
original task. Monitoring does not authorize deployment, publishing, external
messages, purchases, or destructive actions.
