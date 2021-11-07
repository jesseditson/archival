# frozen_string_literal: true

require 'tomlrb'

module Archival
  class Config
    attr_reader :pages_dir, :objects_dir, :assets_dirs, :root, :build_dir,
                :helper_port, :dev_mode

    def initialize(config = {})
      @root = config['root'] || Dir.pwd
      manifest = load_manifest
      @pages_dir = config['pages'] || manifest['pages'] || 'pages'
      @objects_dir = config['objects'] || manifest['objects'] || 'objects'
      @build_dir = config['build_dir'] || manifest['build_dir'] || File.join(
        @root, 'dist'
      )
      @helper_port = config['helper_port'] || manifest['helper_port'] || 2701
      @assets_dirs = config['assets_dirs'] || manifest['assets'] || []
      @dev_mode = config[:dev_mode] || false
    end

    def load_manifest
      manifest_file = File.join(@root, 'manifest.toml')
      return Tomlrb.load_file(manifest_file) if File.file? manifest_file

      {}
    end
  end
end
