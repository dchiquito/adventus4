tree-sitter: 
	cd tree-sitter-adventus \
		&& tree-sitter generate
tree-sitter-test: tree-sitter
	cd tree-sitter-adventus \
		&& tree-sitter parse ../foo.adv
all: tree-sitter tree-sitter-test
