# Maestro shell integration for fish.
function maestro
    command maestro $argv
    set -l __maestro_status $status

    if test $__maestro_status -eq 0
        if test (count $argv) -ge 3; and test "$argv[1]" = "task"; and test "$argv[2]" = "claim"
            set -gx MAESTRO_CURRENT_TASK "$argv[3]"
        else if test (count $argv) -ge 3; and test "$argv[1]" = "task"; and test "$argv[2]" = "complete"
            set -e MAESTRO_CURRENT_TASK
        end
    end

    return $__maestro_status
end
