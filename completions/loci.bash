#!/usr/bin/env bash
# Bash completion for loci
# Install:
#   source completions/loci.bash
#   # or install system-wide:
#   # sudo cp completions/loci.bash /etc/bash_completion.d/loci

_loci() {
    local cur prev words cword
    # Try bash-completion's init first; fallback for minimal environments
    if type _init_completion &>/dev/null; then
        _init_completion || return
    else
        cur="${COMP_WORDS[COMP_CWORD]}"
        prev="${COMP_WORDS[COMP_CWORD-1]}"
        words=("${COMP_WORDS[@]}")
        cword=$COMP_CWORD
    fi

    # If we've already seen `--`, stop completing (args forwarded to tool)
    for ((i = 1; i < cword; i++)); do
        if [[ "${words[i]}" == "--" ]]; then
            return
        fi
    done

    # Complete flags when the current word starts with `-`
    if [[ $cur == -* ]]; then
        local flags=(
            '-l'
            '--list'
            '--json'
        )
        COMPREPLY=($(compgen -W "${flags[*]}" -- "$cur"))
        return
    fi

    # Complete tool names by calling loci in list mode
    if command -v loci &>/dev/null; then
        COMPREPLY=($(compgen -W "$(loci -l 2>/dev/null)" -- "$cur"))
    fi
}

complete -F _loci loci
