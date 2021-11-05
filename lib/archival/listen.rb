# frozen_string_literal: true

require 'listen'
require 'pathname'

module Archival
  def self.child?(parent, child)
    path = Pathname.new(child)
    return true if path.fnmatch?(File.join(parent, '**'))

    false
  end

  def self.change_type(file, _builder)
    if child?(File.join(@config.root, @config.pages_dir), file)
      # a page was modified, rebuild the pages.
      return :pages
    elsif child?(File.join(@config.root, @config.objects_dir), file)
      # an object was modified, rebuild the objects.
      return :objects
    end

    :none
  end

  def self.rebuild(builder, update_objects, update_pages)
    Logger.benchmark('rebuilt') do
      builder.update_objects if update_objects
      builder.update_pages if update_pages
      builder.write_all
    end
  end

  def listen(config)
    @config = Config.new(config)
    builder = Builder.new(config)
    Logger.benchmark('built') do
      builder.write_all
    end
    listener = Listen.to(@config.root) do |modified, added, removed|
      update_pages = false
      update_objects = false
      (modified + added + removed).each do |file|
        case change_type(file, builder)
        when :pages
          update_pages = true
        when :objects
          update_objects = true
        end
      end
      if update_pages || update_objects
        rebuild(builder, update_objects, update_pages)
      end
    end
    listener.start
    listener
  end

  module_function :listen
end
