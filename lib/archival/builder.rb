# frozen_string_literal: true

require 'liquid'
require 'tomlrb'
require 'tags/layout'

Liquid::Template.error_mode = :strict
Liquid::Template.register_tag('layout', Layout)

module Archival
  class Builder
    attr_reader :page_templates

    def initialize(config, *_args)
      refresh_config(config)
    end

    def refresh_config(config)
      @pages_dir = config['pages'] || 'pages'
      @objects_dir = config['objects'] || 'objects'
      @root = config['root'] || Dir.pwd
      @build_dir = config['build_dir'] || File.join(
        @root, 'dist'
      )

      @file_system = Liquid::LocalFileSystem.new(
        @root, '%s.liquid'
      )
      @variables = {}
      @object_types = {}
      @page_templates = {}

      Liquid::Template.file_system = @file_system

      objects_definition_file = File.join(@root,
                                          'objects.toml')
      if File.file? objects_definition_file
        @object_types = read_toml(objects_definition_file)
      end

      update_pages(File.join(@root, @pages_dir))
      update_objects(File.join(@root,
                               @objects_dir))
    end

    def update_pages(dir, prefix = nil)
      add_prefix = lambda { |entry|
        prefix ? File.join(prefix, entry) : entry
      }
      Dir.foreach(dir) do |entry|
        if File.directory? entry
          unless [
            '.', '..'
          ].include?(entry)
            update_pages(File.join(dir, entry),
                         add_prefix(entry))
          end
        elsif File.file? File.join(dir, entry)
          if entry.end_with?('.liquid') && !(entry.start_with? '_')
            page_name = File.basename(entry,
                                      '.liquid')
            template_file = File.join(
              @pages_dir,
              add_prefix.call(page_name)
            )
            content = @file_system.read_template_file(template_file)
            @page_templates[add_prefix.call(page_name)] =
              Liquid::Template.parse(content)
          end
        end
      end
    end

    def update_objects(dir)
      objects = {}
      @object_types.each do |name, _definition|
        objects[name] = []
        obj_dir = File.join(dir, name)
        if File.directory? obj_dir
          Dir.foreach(obj_dir) do |file|
            if file.end_with? '.toml'
              object = read_toml(File.join(
                                   obj_dir, file
                                 ))
              object[:name] =
                File.basename(file, '.toml')
              objects[name].push object
            end
          end
        end
        objects[name] = objects[name].sort do |a, b|
          (a['order'] || a[:name]).to_s <=> (b['order'] || b[:name]).to_s
        end
      end
      @variables['objects'] = objects
    end

    def read_toml(file_path)
      Tomlrb.load_file(file_path)
    end

    def set_var(name, value)
      @variables[name] = value
    end

    def render(page)
      template = @page_templates[page]
      template.render(@variables)
    end

    def write_all
      Dir.mkdir(@build_dir) unless File.exist? @build_dir
      @page_templates.each_key do |template|
        out_dir = File.join(@build_dir,
                            File.dirname(template))
        Dir.mkdir(out_dir) unless File.exist? out_dir
        out_path = File.join(@build_dir,
                             "#{template}.html")
        File.open(out_path, 'w+') do |file|
          file.write(render(template))
        end
      end
    end
  end
end
