# Pitfalls that cost time once already

- **Never overwrite a loaded `.so` in place.** `cp` onto `libSkirmishAI.so` while a game had it mapped crashed the user's GUI
  game 24 s later (SIGSEGV, PC=0 on the main thread; coredumpctl). Installs replace by rename (`run/install_ai.sh`), and the
  BAR install holds its own copy, never a symlink to the development build.
- **`pkill -f <pattern>` / `pgrep -f` match the invoking shell's own command line** and kill it. Use PIDs or `pkill -x`.
- **The engine log is block-buffered.** After SIGTERM it stops wherever the buffer last flushed (it misled the first smoke
  test at frame 31). End matches with `/kill`.
- **Factory build orders: SHIFT means "build five".** Enforced in the shim; see K-rules-factory-shift-means-five.
- **The arena rebuilds the workspace at start.** Do not leave `crates/` half-edited while a batch is starting.
- **12 matches is noise-level.** ±14 points at 50%. Confirm with 24+ before believing a gain.
- After a bot restart the brain takes the commander's current position as home; brain state is not persisted.
