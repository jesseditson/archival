require 'liquid'
require 'tomlrb'
require 'tags/layout'

Liquid::Template.error_mode = :strict
Liquid::Template.register_tag("layout", Layout)

class Build
    attr_reader :page_templates

    def initialize(config, *args)
        @pages_dir = config["pages"] || "pages"
        @objects_dir = config["objects"] || "objects"
        @root = config["root"] || Dir.pwd

        @file_system = Liquid::LocalFileSystem.new(@root, "%s.liquid")
        @variables = {}
        @object_types = {}
        @page_templates = {}

        Liquid::Template.file_system = @file_system

        objects_definition_file = File.join(@root, "objects.toml")
        if File.file? objects_definition_file
            @object_types = read_toml(objects_definition_file)
        end

        add_pages(File.join(@root, @pages_dir))
        add_objects(File.join(@root, @objects_dir))
    end

    def add_pages(dir, prefix = nil)
        add_prefix = -> (entry) { prefix ? File.join(prefix, entry) : entry }
        Dir.foreach(dir) { |entry|
            if File.directory? entry
                unless entry == "." or entry == ".."
                    add_pages(File.join(dir, entry), add_prefix(entry))
                end
            elsif File.file? File.join(dir, entry)
                if entry.end_with? ".liquid"
                    unless entry.start_with? "_"
                        page_name = File.basename(entry, ".liquid")
                        content = @file_system.read_template_file(File.join(@pages_dir, add_prefix.(page_name)))
                        @page_templates[add_prefix.(page_name)] = Liquid::Template.parse(content)
                    end
                end
            end
        }
    end

    def add_objects(dir)
        objects = {}
        @object_types.each { |name, definition|
            objects[name] = []
            obj_dir = File.join(dir, name)
            if File.directory? obj_dir
                Dir.foreach(obj_dir) { |file|
                    if file.end_with? ".toml"
                        object = read_toml(File.join(obj_dir, file))
                        object[:name] = File.basename(file, ".toml")
                        objects[name].push object
                    end
                }
            end
            objects[name] = objects[name].sort { |a, b| "#{a["order"] || a[:name]}" <=> "#{b["order"] || b[:name]}" }
        }
        @variables["objects"] = objects
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
end