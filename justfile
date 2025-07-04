@_usage:
	just -l

# Increase version & tag all artifacts
release:
    cargo workspaces version -a --force '*' --tag-prefix '' --no-individual-tags
