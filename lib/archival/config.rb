# frozen_string_literal: true

module Archival
  class Config
    attr_reader :pages_dir, :objects_dir, :root, :build_dir, :helper_port,
                :dev_mode

    def initialize(config = {})
      @pages_dir = config['pages'] || 'pages'
      @objects_dir = config['objects'] || 'objects'
      @root = config['root'] || Dir.pwd
      @build_dir = config['build_dir'] || File.join(
        @root, 'dist'
      )
      @helper_port = config['helper_port'] || 2701
      @dev_mode = config[:dev_mode] || false
    end
  end
end
