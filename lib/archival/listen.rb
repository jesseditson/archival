# frozen_string_literal: true

require 'listen'
require 'pathname'

module Archival
  def listen(config)
    @config = Config.new(config, true)
    builder = Builder.new(@config)
    Logger.benchmark('built') do
      builder.write_all
    end
    listen_paths = %r{(#{@config.pages_dir}|#{@config.objects_dir})/}
    listener = Listen.to(@config.root,
                         only: listen_paths) do |modified, added, removed|
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
        puts @server.socket
      end
    end
    listener.start
    @server = serve_helpers
  end

  module_function :listen

  class << self
    private

    def child?(parent, child)
      path = Pathname.new(child)
      return true if path.fnmatch?(File.join(parent, '**'))

      false
    end

    def change_type(file, _builder)
      # a page was modified, rebuild the pages.
      return :pages if child?(File.join(@config.root, @config.pages_dir),
                              file)
      # an object was modified, rebuild the objects.
      return :objects if child?(File.join(@config.root, @config.objects_dir),
                                file)

      :none
    end

    def rebuild(builder, update_objects, update_pages)
      Logger.benchmark('rebuilt') do
        builder.update_objects if update_objects
        builder.update_pages if update_pages
        builder.write_all
      end
    end

    def serve_helpers
      server = HelperServer.new(@config.helper_port)
      server.start
      server
    end
  end
end
