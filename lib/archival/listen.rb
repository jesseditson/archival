# frozen_string_literal: true

require 'listen'
require 'pathname'

module Archival
  Change = Struct.new(:path, :type)

  def listen(config = {})
    @config = Config.new(config.merge(dev_mode: true))
    builder = Builder.new(@config)
    Logger.benchmark('built') do
      builder.write_all
    end
    ignore = %r{/dist/}
    listener = Listen.to(@config.root,
                         ignore: ignore) do |modified, added, removed|
      changes = {
        pages: [],
        objects: [],
        assets: [],
        layout: [],
        config: []
      }
      add_change = lambda { |file, type|
        c_type = change_type(file)
        changes[c_type] << change(file, type) unless c_type == :none
      }
      added.each do |file|
        add_change.call(file, :added)
      end
      modified.each do |file|
        add_change.call(file, :modified)
      end
      removed.each do |file|
        add_change.call(file, :removed)
      end
      @server.refresh_client if rebuild?(builder, changes)
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

    def change(file, type)
      c = Change.new
      c.path = Pathname.new(file).relative_path_from(@config.root)
      c.type = type
      c
    end

    def change_type(file)
      # a page was modified, rebuild the pages.
      return :pages if child?(File.join(@config.root, @config.pages_dir),
                              file)
      # an object was modified, rebuild the objects.
      return :objects if child?(File.join(@config.root, @config.objects_dir),
                                file)

      # an asset was changed, which just means to copy or delete it
      @config.assets_dirs.each do |dir|
        return :assets if child?(File.join(@config.root, dir), file)
      end

      # a static file was changed - copy or delete those too.
      return :assets if child?(File.join(@config.root, @config.static_dir),
                               file)

      # other special files
      return :layout if child?(File.join(@config.root, 'layout'), file)
      return :config if ['manifest.toml',
                         'objects.toml'].include? File.basename(file)

      :none
    end

    def rebuild?(builder, changes)
      return false if changes.values.all?(&:empty?)

      Logger.benchmark('rebuilt') do
        if changes[:pages].length || changes[:objects].length
          builder.update_pages(changes[:pages], changes[:objects])
        end
        builder.update_assets(changes[:assets]) if changes[:assets].length
        if changes[:assets].length || changes[:layouts] || changes[:config]
          # TODO: optimization: this could operate on the known subset of
          # changes instead...
          builder.full_rebuild
        end
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
