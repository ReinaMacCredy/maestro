# Maestro shell integration for bash/zsh.
maestro() {
  local __maestro_status

  command maestro "$@"
  __maestro_status=$?

  if [ "$__maestro_status" -eq 0 ]; then
    if [ "$1" = "task" ] && [ "$2" = "claim" ] && [ -n "${3-}" ]; then
      export MAESTRO_CURRENT_TASK="$3"
    elif [ "$1" = "task" ] && [ "$2" = "complete" ] && [ -n "${3-}" ]; then
      unset MAESTRO_CURRENT_TASK
    fi
  fi

  return "$__maestro_status"
}
