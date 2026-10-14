# 11: Signal the child's process group without shelling out to `kill`

Type: task
Status: ready-for-agent
Severity: medium
Area: adapters/spawn
Verified: yes — `src/adapters/spawn.rs:216-228` runs `kill -s KILL -- -<pgid>` and falls back to killing only the direct child when `kill` is unavailable.

## Symptom

The timeout's whole purpose is to stop a forked grandchild (mise forks). When the `kill` binary is absent — a minimal or distroless image — the fallback kills only the direct child, the grandchild keeps the pipes open, and the call hangs again. `unsafe` is forbidden crate-wide, which is why the code shells out at all.

## Suggested direction

Use a crate that wraps the syscall safely — `rustix` (`rustix::process::kill_process_group`) is the lean choice; `nix` is the more common one. Replace `kill_group`'s shell-out with the syscall, keeping the direct-child fallback. Verify the existing `a_forking_child_is_bounded_by_the_timeout_and_its_group_is_killed` test still passes. This also drops a dependency on an external binary.

## Related

- 04, Group D.
