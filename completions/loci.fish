# Fish completion for loci
# Install:
#   source completions/loci.fish
#   # or install permanently:
#   # cp completions/loci.fish ~/.config/fish/completions/loci.fish

function __fish_loci_list_tools
    loci -l 2>/dev/null
end

function __fish_loci_no_dashdash
    not __fish_seen_argument -- '--'
end

# Flags
complete -c loci -n '__fish_loci_no_dashdash' -s l -l list -d 'List executables and exit'
complete -c loci -n '__fish_loci_no_dashdash' -l json -d 'JSON output format (combine with -l)'

# Tool names (dynamic, from loci -l)
complete -c loci -n '__fish_loci_no_dashdash && not __fish_seen_argument -s l -l list' -k -a '(__fish_loci_list_tools)' -d 'Tool name'
