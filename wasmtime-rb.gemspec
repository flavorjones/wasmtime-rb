# frozen_string_literal: true

begin
  require_relative "lib/wasmtime/version"
rescue LoadError
  puts "WARNING: Could not load Wasmtime::VERSION" # this will happen during smoke tests in CI
end

Gem::Specification.new do |spec|
  spec.name = "wasmtime-rb"
  spec.version = defined?(Wasmtime::VERSION) ? Wasmtime::VERSION : "0.0.0"
  spec.authors = ["Ian Ker-Seymer"]
  spec.email = ["hello@ianks.com"]

  spec.summary = "Wasmtime bindings for Ruby"
  spec.description = "A Ruby binding for Wasmtime, a WebAssembly runtime."
  spec.homepage = "https://github.com/BytecodeAlliance/wasmtime-rb"
  spec.license = "Apache-2.0"
  spec.required_ruby_version = ">= 2.7.0"

  spec.metadata["homepage_uri"] = "https://github.com/BytecodeAlliance/wasmtime-rb"
  spec.metadata["source_code_uri"] = "https://github.com/BytecodeAlliance/wasmtime-rb"
  spec.metadata["cargo_crate_name"] = "ext"
  spec.metadata["changelog_uri"] = "https://github.com/bytecodealliance/wasmtime-rb/blob/main/CHANGELOG.md"

  spec.files = Dir["{lib,ext}/**/*", "LICENSE", "README.md", "Cargo.*"]
  spec.bindir = "exe"
  spec.executables = spec.files.grep(%r{\Aexe/}) { |f| File.basename(f) }
  spec.require_paths = ["lib"]

  spec.extensions = ["ext/extconf.rb"] # Future: ["ext/Cargo.toml"] with rubygems >= 3.3.24

  # Can be removed for binary gems and rubygems >= 3.3.24
  spec.add_dependency "rb_sys", "~> 0.9.34"
end
