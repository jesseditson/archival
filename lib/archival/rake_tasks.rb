# frozen_string_literal: true

require 'rake'
require 'rake/tasklib'
require 'archival'

class RakeTasks
  # Common tasks for archival.
  #
  # To include, just add
  #
  # require 'archival/rake_tasks'
  #
  # to your Rakefile.
  include Rake::DSL if defined? Rake::DSL

  class << self
    # set when install'd.
    attr_accessor :instance

    def install_tasks
      new.install
    end
  end

  def install
    build_dir = Dir.pwd

    task 'build' do
      Archival::Logger.benchmark('built') do
        config = Config.new('root' => build_dir)
        builder = Archival::Builder.new(config)
        builder.write_all
      end
    end

    task 'run' do
      Archival.listen(build_dir)
      begin
        sleep
      rescue Interrupt
        # Don't print a stack when a user interrupts, as this is the right way
        # to stop the development server.
        puts ''
      end
    end

    RakeTasks.instance = self
  end
end

RakeTasks.install_tasks
