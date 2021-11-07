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
    ignore = %r{/dist/}
    listener = Listen.to(@config.root,
                         ignore: ignore) do |modified, added, removed|
      updated_pages = []
      updated_objects = []
      updated_assets = []
      (modified + added + removed).each do |file|
        case change_type(file)
        when :pages
          updated_pages << file
        when :objects
          updated_objects << file
        when :assets
          updated_assets << file
        end
      end
      @server.refresh_client if rebuild?(builder, updated_objects,
                                         updated_pages, updated_assets)
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

    def change_type(file)
      # a page was modified, rebuild the pages.
      return :pages if child?(File.join(@config.root, @config.pages_dir),
                              file)
      # an object was modified, rebuild the objects.
      return :objects if child?(File.join(@config.root, @config.objects_dir),
                                file)

      # layout and other assets. For now, this is everything.
      @config.assets_dirs.each do |dir|
        return :assets if child?(File.join(@config.root, dir), file)
      end
      return :assets if child?(File.join(@config.root, "layout"), file)
      return :assets if ['manifest.toml',
                         'objects.toml'].include? File.basename(file)

      :none
    end

    def rebuild?(builder, updated_objects, updated_pages, updated_assets)
      if updated_pages.empty? && updated_objects.empty? && updated_assets.empty?
        return false
      end

      Logger.benchmark('rebuilt') do
        builder.update_objects if updated_objects.length
        builder.update_pages if updated_pages.length
        builder.full_rebuild if updated_assets.length
        builder.write_all
      end
      true
    end

    def serve_helpers
      @server = HelperServer.new(@config.helper_port, @config.build_dir)
      @server.start
    end
  end
end
