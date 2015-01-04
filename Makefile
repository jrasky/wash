# run main target like this because make handles SIGINT correctly
h:
	cargo build
	target/wash

n:
	cargo run --bin key_helper
