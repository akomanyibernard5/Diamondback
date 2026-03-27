.PHONY: clean test test-errors

TESTS := $(patsubst test/%.snek,test/%.run,$(wildcard test/*.snek))

test/%.s: test/%.snek src/main.rs Cargo.toml
	cargo run -- $< $@

test/%.run: test/%.s runtime/start.rs
	nasm -f macho64 $< -o runtime/our_code.o
	ar rcs runtime/libour_code.a runtime/our_code.o
	rustc --target x86_64-apple-darwin -L runtime runtime/start.rs -o $@

# Build and run all valid test programs
test: $(TESTS)
	@for bin in $(TESTS); do \
		echo -n "$$bin: "; \
		./$$bin false; \
	done

# Verify that error programs produce the expected panic messages
test-errors:
	@for f in test/error/*.snek; do \
		name=$$(basename $$f .snek); \
		result=$$(cargo run -- $$f /dev/null 2>&1 | grep -A1 'panicked at' | tail -1 | xargs); \
		echo "error/$$name: $$result"; \
	done

clean:
	rm -f test/*.s test/*.run runtime/*.o runtime/*.a
