# Bash completion for kotonoha (言の葉)
_kotonoha_completions() {
    local cur prev
    if declare -F _init_completion >/dev/null 2>&1; then
        _init_completion -n = || return
    else
        cur="${COMP_WORDS[COMP_CWORD]}"
        prev="${COMP_WORDS[COMP_CWORD-1]}"
    fi

    local long_opts="
        --bundle
        --bundles
        --clean-bundled
        --clean-sources
        --clean
        --config
        --show-config
        --inspect
        --manage-known
        --manage-mined
        --manage-ignored
        --sync
        --force
        --version
        --help
        --completions
    "

    local short_opts="-b -B -C -c -S -i -k -m -I -s -f -v -h"
    local subcommands="bundle bundles"

    case "${prev}" in
        --bundle|-b|bundle|--inspect|-i)
            if [[ "${cur}" == -* ]]; then
                COMPREPLY=($(compgen -W "--force -f --help -h" -- "${cur}"))
                return 0
            fi
            COMPREPLY=($(compgen -f -- "${cur}"))
            return 0
            ;;
        --completions)
            COMPREPLY=($(compgen -W "bash zsh fish" -- "${cur}"))
            return 0
            ;;
        --config|-c|--show-config|-S|--manage-known|-k|--manage-mined|-m|--manage-ignored|-I|--sync|-s|--clean-bundled|-C|--bundles|-B|bundles|--version|-v|-V)
            return 0
            ;;
    esac

    if [[ "${cur}" == --* ]]; then
        COMPREPLY=($(compgen -W "${long_opts}" -- "${cur}"))
        return 0
    elif [[ "${cur}" == -* ]]; then
        COMPREPLY=($(compgen -W "${long_opts} ${short_opts}" -- "${cur}"))
        return 0
    fi

    local all_candidates="${long_opts} ${short_opts} ${subcommands}"
    local opt_matches=($(compgen -W "${all_candidates}" -- "${cur}"))
    local file_matches=($(compgen -f -- "${cur}"))
    COMPREPLY=("${opt_matches[@]}" "${file_matches[@]}")
}

complete -o default -o bashdefault -o filenames -F _kotonoha_completions kotonoha
