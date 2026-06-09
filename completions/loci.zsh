#compdef loci
# Zsh completion for loci
# Install:
#   source completions/loci.zsh
#   # or add to fpath:
#   # cp completions/loci.zsh /usr/share/zsh/site-functions/_loci

_loci() {
    local -a flags
    flags=(
        '-l[list mode (print executables and exit)]'
        '--list[list mode (print executables and exit)]'
        '--json[JSON output format (combine with -l)]'
    )

    # If `--` was already seen, stop completing
    if [[ ${words[(r)--]} == "--" ]]; then
        return
    fi

    # Current word starts with `-` → offer flags
    # Use $words[$CURRENT] to check the actual word being completed
    local cur="${words[$CURRENT]}"
    if [[ $cur == -* ]]; then
        _describe 'flags' flags
        return
    fi

    # Otherwise complete tool names from `loci -l`
    local -a tools
    if (( $+commands[loci] )); then
        tools=(${(f)"$(loci -l 2>/dev/null)"})
        _describe 'tools' tools
    fi
}

_loci "$@"
