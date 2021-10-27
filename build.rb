#!/usr/bin/env ruby

require_relative 'lib/builder'

build_dir = ARGV[0] || Dir.pwd

builder = Builder.new("root" => build_dir)

builder.write_all()