# frozen_string_literal: true

require 'listen'
require 'archival/version'
require 'archival/builder'

module Archival
  # Main Archival module. See https://archival.dev for docs.

  def self.listen(build_dir)
    builder = Builder('root' => build_dir)
    listener = Listen.to(build_dir) do |modified, added, removed|
      puts(modified: modified, added: added,
           removed: removed)
      # if an object was modified, rebuild the objects
      # if a page was modified, rebuild the pages
    end
    listener.start
    listener
  end
end
