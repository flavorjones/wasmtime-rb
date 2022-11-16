# frozen_string_literal: true

require_relative "wasmtime/version"

begin
  ruby_version = /(\d+\.\d+)/.match(::RUBY_VERSION)
  require_relative "wasmtime/#{ruby_version}/ext"
rescue LoadError
  require "wasmtime/ext"
end

module Wasmtime
  class Error < StandardError; end
  # Your code goes here...
end
