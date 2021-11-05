# frozen_string_literal: true

require 'benchmark'

module Archival
  class Logger
    def self.benchmark(message, &block)
      Benchmark.bm do |bm|
        bm.report(message, &block)
      end
    end
  end
end
