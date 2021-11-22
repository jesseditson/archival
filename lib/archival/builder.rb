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
      @variables = {}
      @object_types = {}
      @page_templates = {}
      @dynamic_pages = Set.new
      @dynamic_templates = {}
      @parser = Archival::Parser.new(pages_dir)

      Liquid::Template.file_system = Liquid::LocalFileSystem.new(
        pages_dir, '_%s.liquid'
      )

      objects_definition_file = File.join(@config.root,
                                          'objects.toml')
      if File.file? objects_definition_file
        @object_types = Tomlrb.load_file(objects_definition_file)
      end

      update_pages
    end

    def full_rebuild
      Layout.reset_cache
      refresh_config
    end

    def update_pages
      @object_types.each do |name, definition|
        read_objects name do |_object|
          is_template = definition.key? 'template'
          @dynamic_pages << definition['template'] if is_template
        end
      end
      do_update_pages(pages_dir)
    end

    def dynamic?(file)
      @dynamic_pages.include? File.basename(file, '.liquid')
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

    def read_objects(name)
      obj_dir = File.join(objects_dir, name)
      if File.directory? obj_dir
        Dir.foreach(obj_dir) do |file|
          if file.end_with? '.toml'
            object = Tomlrb.load_file(File.join(
                                        obj_dir, file
                                      ))
            object[:name] =
              File.basename(file, '.toml')
            yield object
          end
        end
      end
    end

    def objects_for_template(template)
      objects = {}
      @object_types.each do |name, definition|
        objects[name] = {}
        read_objects name do |object|
          objects[name][object[:name]] =
            @parser.parse_object(object, definition, template)
        end
        objects[name] = sort_objects(objects[name])
      end
      @dynamic_pages.each do |page|
        objects[page].each do |obj|
          path = Pathname.new(File.join(pages_dir, page,
                                        "#{obj[:name]}.html"))
          objects[page][obj[:name]]['path'] =
            path.relative_path_from(objects_dir).to_s
        end
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
      vars = @variables.clone
                       .merge(objects_for_template(template_path))
                       .merge('template_path' => template_path)
      template.render(vars)
    end

    def render_dynamic(page, obj)
      dir = File.join(pages_dir, page)
      template = @dynamic_templates[page]
      template_path = File.join(dir, obj[:name])
      vars = @variables.clone
                       .merge(objects_for_template(template_path))
                       .merge({ page => obj })
                       .merge('template_path' => template_path)
      puts 'TEMPLATE'
      puts template.render(vars)
      puts '----TEMPLATE'
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
      @dynamic_pages.each do |page|
        out_dir = File.join(@config.build_dir, page)
        Dir.mkdir(out_dir) unless File.exist? out_dir
        read_objects(page) do |obj|
          out_path = File.join(out_dir, "#{obj[:name]}.html")
          File.open(out_path, 'w+') do |file|
            file.write(render_dynamic(page, obj))
          end
        end
      end
      return if @config.dev_mode

      # in production, also copy all assets to the dist folder.
      @config.assets_dirs.each do |asset_dir|
        FileUtils.copy_entry File.join(@config.root, asset_dir),
                             File.join(@config.build_dir, asset_dir)
      end
    end

    private

    def dev_mode_content
      "<script src=\"http://localhost:#{@config.helper_port}/js/archival-helper.js\" type=\"application/javascript\"></script>" # rubocop:disable Layout/LineLength
    end
  end
end
