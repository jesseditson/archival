# frozen_string_literal: true

require 'listen'
require 'pathname'

module Archival
  def self.child?(parent, child)
    path = Pathname.new(child)
    return true if path.fnmatch?(File.join(parent, '**'))

    false
  end

  def self.process_change?(file, builder)
    if child?(File.join(@config.root, @config.pages_dir), file)
      # a page was modified, rebuild the pages.
      builder.update_pages
      return true
    elsif child?(File.join(@config.root, @config.objects_dir), file)
      # an object was modified, rebuild the objects.
      builder.update_objects
      return true
    end
    false
  end

  def listen(config)
    @config = Config.new(config)
    builder = Builder.new(config)
    builder.write_all
    listener = Listen.to(@config.root) do |modified, added, removed|
      needs_update = false
      (modified + added + removed).each do |file|
        needs_update = true if process_change?(file, builder)
      end
      builder.write_all if needs_update
    end
    listener.start
    listener
  end

  module_function :listen
end
