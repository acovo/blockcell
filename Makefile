.PHONY: release reload

release:
	cd webui && npm ci && npm test && npm run build
	cargo build --release --locked
	@set -eu; \
	install_dir="$(HOME)/.local/bin"; \
	mkdir -p "$$install_dir"; \
	temporary_path="$$install_dir/.blockcell.tmp.$$$$"; \
	trap 'rm -f "$$temporary_path"' EXIT; \
	cp target/release/blockcell "$$temporary_path"; \
	chmod +x "$$temporary_path"; \
	mv -f "$$temporary_path" "$$install_dir/blockcell"; \
	trap - EXIT; \
	"$$install_dir/blockcell" --version


reload:
	cp -r skills/* ~/.blockcell/workspace/skills/ || true
	cargo run -p blockcell -- skills reload && \
	blockcell --version
