_kotonoha_completions() {
    local cur="${COMP_WORDS[COMP_CWORD]}"
    local flags="--config --show-config --backfill-translations --inspect --manage-known --manage-ignored --clear-cache --sync --version --help"

    if [[ "${cur}" == --* ]]; then
        COMPREPLY=($(compgen -W "${flags}" -- "${cur}"))
        return
    fi

    # Default: complete with files (for <MEDIA_FILE> argument)
    COMPREPLY=($(compgen -f -- "${cur}"))
}

complete -F _kotonoha_completions kotonoha
