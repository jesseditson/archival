# frozen_string_literal: true

require 'listen'

module Archival
  def listen(build_dir)
    builder = Builder.new('root' => build_dir)
    listener = Listen.to(build_dir) do |modified, added, removed|
      puts(modified: modified, added: added,
           removed: removed)
      # if an object was modified, rebuild the objects
      # if a page was modified, rebuild the pages
    end
    listener.start
    listener
  end

  module_function :listen
end
