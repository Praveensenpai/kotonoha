# Fish completion for kotonoha
complete -c kotonoha -s b -l bundle -d "Pre-save video into lightweight .koto package" -r -F
complete -c kotonoha -s B -l bundles -d "View, inspect and manage saved .koto bundles"
complete -c kotonoha -s C -l clean-bundled -l clean-sources -l clean -d "Remove original source files of bundled media"
complete -c kotonoha -s c -l config -d "Interactive TUI configuration manager"
complete -c kotonoha -s S -l show-config -d "Display active configuration settings"
complete -c kotonoha -s i -l inspect -d "Inspect sentences in media/koto file" -r -F
complete -c kotonoha -s k -l manage-known -d "View & remove words from known database"
complete -c kotonoha -s m -l manage-mined -d "View & remove words from mined list"
complete -c kotonoha -s I -l manage-ignored -d "View & remove words from ignore list"
complete -c kotonoha -s s -l sync -d "Push locally mined cards to Anki"
complete -c kotonoha -s f -l force -d "Force re-bundling or overwriting"
complete -c kotonoha -s v -s V -l version -d "Print version information"
complete -c kotonoha -s h -l help -d "Show help information"
complete -c kotonoha -l completions -d "Generate shell completions" -x -a "bash zsh fish"
