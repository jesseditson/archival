# frozen_string_literal: true

require 'listen'
require 'pathname'

module Archival
  def listen(config = {})
    @config = Config.new(config.merge(dev_mode: true))
    builder = Builder.new(@config)
    Logger.benchmark('built') do
      builder.write_all
    end
    listen_paths = %r{(#{@config.pages_dir}|#{@config.objects_dir})/}
    listener = Listen.to(@config.root,
                         only: listen_paths) do |modified, added, removed|
      updated_pages = []
      updated_objects = []
      (modified + added + removed).each do |file|
        case change_type(file, builder)
        when :pages
          updated_pages << file
        when :objects
          updated_objects << file
        end
      end
      if updated_pages.length || updated_objects.length
        rebuild(builder, updated_objects, updated_pages)
        @server.refresh_client
      end
    end
    listener.start
    serve_helpers
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

    def rebuild(builder, updated_objects, updated_pages)
      Logger.benchmark('rebuilt') do
        builder.update_objects if updated_objects.length
        builder.update_pages if updated_pages.length
        builder.write_all
      end
    end

    def serve_helpers
      @server = HelperServer.new(@config.helper_port, @config.build_dir)
      @server.start
    end
  end
end
