# frozen_string_literal: true

require 'liquid'
require 'tomlrb'
require 'redcarpet'

module Archival
  class DuplicateKeyError < StandardError
  end

  class Builder
    attr_reader :page_templates

    def initialize(config, *_args)
      @config = config
      refresh_config
    end

    def pages_dir
      File.join(@config.root, @config.pages_dir)
    end

    def objects_dir
      File.join(@config.root, @config.objects_dir)
    end

    def refresh_config
      @file_system = Liquid::LocalFileSystem.new(
        pages_dir, '%s.liquid'
      )
      @object_types = {}
      @page_templates = {}
      @dynamic_types = Set.new
      @dynamic_templates = {}
      @parser = Archival::Parser.new(pages_dir)

      Liquid::Template.file_system = Liquid::LocalFileSystem.new(
        pages_dir, '_%s.liquid'
      )

      @objects_definition_file = File.join(@config.root, 'objects.toml')

      update_pages
    end

    def full_rebuild
      Layout.reset_cache
      refresh_config
    end

    def update_objects(_updated_objects = nil)
      @object_types = {}
      if File.file? @objects_definition_file
        @object_types = Tomlrb.load_file(@objects_definition_file)
      end
      @dynamic_types = Set.new
      @object_types.each do |_name, definition|
        is_template = definition.key? 'template'
        @dynamic_types << definition['template'] if is_template
      end
      # TODO: remove deleted dynamic pages
    end

    def update_pages(_updated_pages = nil, _updated_objects = nil)
      update_objects
      # TODO: remove deleted pages
      do_update_pages(pages_dir)
    end

    def update_assets(changes)
      changes.each do |change|
        asset_path = File.join(@config.build_dir, change.path)
        case change.type
        when :removed
          FileUtils.rm_rf asset_path
        else
          puts change.path
          FileUtils.copy_entry File.join(@config.root, change.path), asset_path
        end
      end
    end

    def dynamic?(file)
      @dynamic_types.include? File.basename(file, '.liquid')
    end

    def template_for_page(template_file)
      content = @file_system.read_template_file(template_file)
      content += dev_mode_content if @config.dev_mode
      Liquid::Template.parse(content)
    end

    def do_update_pages(dir, prefix = nil)
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
          page_name = File.basename(entry, '.liquid')
          template_file = add_prefix.call(page_name)
          if dynamic? entry
            @dynamic_templates[template_file] = template_for_page(template_file)
          elsif entry.end_with?('.liquid') && !(entry.start_with? '_')
            @page_templates[template_file] =
              template_for_page(template_file)
          end
        end
      end
    end

    def read_objects(type)
      obj_dir = File.join(objects_dir, type)
      return unless File.directory? obj_dir

      Dir.foreach(obj_dir) do |file|
        if file.end_with? '.toml'
          object = Tomlrb.load_file(File.join(
                                      obj_dir, file
                                    ))
          object[:name] =
            File.basename(file, '.toml')
          yield object[:name], object
        end
      end
    end

    def path_for_template(name, type)
      Pathname.new(File.join(pages_dir, type, "#{name}.html"))
    end

    def objects_for_template(template_path)
      objects = {}
      @object_types.each do |type, definition|
        objects[type] = {}
        is_dynamic = @dynamic_types.include? type
        read_objects type do |name, object|
          objects[type][name] = @parser.parse_object(
            object, definition, template_path
          )
          if is_dynamic
            path = path_for_template(name, type)
            objects[type][name]['path'] =
              path.relative_path_from(File.dirname(template_path)).to_s
          end
        end
        objects[type] = sort_objects(objects[type])
      end
      objects
    end

    def sort_objects(objects)
      # Sort by either 'order' key or object name, depending on what is
      # available.
      sorted_by_keys = objects.sort_by do |name, obj|
        obj.key?('order') ? obj['order'].to_s : name
      end
      sorted_objects = Archival::TemplateArray.new
      sorted_by_keys.each do |d|
        raise DuplicateKeyError if sorted_objects.key?(d[0])

        sorted_objects.push(d[1])
        sorted_objects[d[0]] = d[1]
      end
      sorted_objects
    end

    def render(page)
      dir = File.join(pages_dir, File.dirname(page))
      template = @page_templates[page]
      template_path = File.join(dir, page)
      parsed_objects = objects_for_template(template_path)
      template.render('objects' => parsed_objects,
                      'template_path' => template_path)
    end

    def render_dynamic(type, name)
      dir = File.join(pages_dir, type)
      template = @dynamic_templates[type]
      template_path = File.join(dir, name)
      parsed_objects = objects_for_template(template_path)
      obj = parsed_objects[type][name]
      vars = {}
             .merge(
               'objects' => parsed_objects,
               'template_path' => template_path
             )
             .merge({ type => obj })
      template.render(vars)
    end

    def write_all
      Dir.mkdir(@config.build_dir) unless File.exist? @config.build_dir
      @page_templates.each_key do |template|
        out_dir = File.join(@config.build_dir,
                            File.dirname(template))
        Dir.mkdir(out_dir) unless File.exist? out_dir
        out_path = File.join(out_dir, "#{template}.html")
        File.open(out_path, 'w+') do |file|
          file.write(render(template))
        end
      end
      @dynamic_types.each do |type|
        out_dir = File.join(@config.build_dir, type)
        Dir.mkdir(out_dir) unless File.exist? out_dir
        read_objects(type) do |name|
          out_path = File.join(out_dir, "#{name}.html")
          File.open(out_path, 'w+') do |file|
            file.write(render_dynamic(type, name))
          end
        end
      end

      # in production (or init), also copy all assets to the dist folder.
      # in dev, they will be copied as they change.
      @config.assets_dirs.each do |asset_dir|
        asset_path = File.join(@config.build_dir, asset_dir)
        next if @config.dev_mode || !File.exist?(asset_path)

        FileUtils.copy_entry File.join(@config.root, asset_dir), asset_path
      end
    end

    private

    def dev_mode_content
      "<script src=\"http://localhost:#{@config.helper_port}/js/archival-helper.js\" type=\"application/javascript\"></script>" # rubocop:disable Layout/LineLength
    end
  end
end
