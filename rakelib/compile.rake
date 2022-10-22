require "rake/extensiontask"

GEMSPEC = Bundler.load_gemspec("wasmtime-rb.gemspec")

CROSS_PLATFORMS = [
  "aarch64-linux",
  "arm64-darwin",
  "x64-mingw-ucrt",
  "x64-mingw32",
  "x86_64-darwin",
  "x86_64-linux",
  "x86_64-linux-musl"
]

Rake::ExtensionTask.new("ext", GEMSPEC) do |ext|
  ext.lib_dir = "lib/wasmtime"
  ext.ext_dir = "ext"
  ext.cross_compile = ENV.key?("RUST_TARGET")
  ext.cross_platform = CROSS_PLATFORMS
end
