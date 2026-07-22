import { useState } from 'react';
import { X, ChevronDown, ChevronUp } from 'lucide-react';

export interface TaskItem {
  id: string;
  name: string;
  status: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';
  percent: number;
  speed?: string;
  remaining?: string;
}

interface BatchProgressPanelProps {
  tasks: TaskItem[];
  onCancel: (id: string) => void;
  onCancelAll: () => void;
  onTogglePause?: (id: string) => void;
}

export function BatchProgressPanel({
  tasks,
  onCancel,
  onCancelAll,
}: BatchProgressPanelProps) {
  const [collapsed, setCollapsed] = useState(false);

  if (tasks.length === 0) return null;

  const runningCount = tasks.filter((t) => t.status === 'running').length;
  const completedCount = tasks.filter((t) => t.status === 'completed').length;
  const failedCount = tasks.filter((t) => t.status === 'failed').length;

  return (
    <div className="batch-progress-panel bg-[#152535] rounded-lg border border-[#2d3a4f] mb-4">
      <div className="batch-progress-header flex items-center justify-between p-3 border-b border-[#2d3a4f]">
        <div className="flex items-center gap-3">
          <span className="font-medium text-sm">上传任务</span>
          <span className="text-xs text-[#8b9cb3]">
            运行 {runningCount} · 完成 {completedCount} · 失败 {failedCount}
          </span>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => setCollapsed(!collapsed)}
            className="p-1 hover:bg-[#243044] rounded transition-colors"
            aria-label={collapsed ? '展开' : '收起'}
          >
            {collapsed ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
          </button>
          <button
            onClick={onCancelAll}
            className="p-1 hover:bg-[#243044] rounded transition-colors"
            aria-label="全部取消"
          >
            <X size={16} />
          </button>
        </div>
      </div>
      {!collapsed && (
        <div className="batch-progress-list max-h-48 overflow-y-auto">
          {tasks.map((task) => (
            <div
              key={task.id}
              className="batch-progress-item p-3 border-b border-[#2d3a4f] last:border-b-0"
            >
              <div className="flex justify-between items-center text-sm mb-2">
                <span className="truncate text-[#e7ecf3] max-w-[200px]">{task.name}</span>
                {task.status !== 'completed' && task.status !== 'cancelled' && (
                  <button
                    onClick={() => onCancel(task.id)}
                    className="p-1 hover:bg-[#243044] rounded transition-colors text-[#8b9cb3] hover:text-[#e7ecf3]"
                    aria-label={`取消 ${task.name}`}
                  >
                    <X size={14} />
                  </button>
                )}
              </div>
              <div className="progress-bar h-1.5 bg-[#0f1419] rounded-full overflow-hidden mb-1">
                <div
                  className={`h-full transition-all duration-200 ${
                    task.status === 'completed'
                      ? 'bg-green-500'
                      : task.status === 'failed'
                        ? 'bg-red-500'
                        : 'bg-gradient-to-r from-[#2aabee] to-[#4ade80]'
                  }`}
                  style={{ width: `${task.percent}%` }}
                />
              </div>
              <div className="text-xs text-[#8b9cb3]">
                {task.status === 'running' && task.speed && (
                  <span>
                    {task.speed}
                    {task.remaining && ` · ${task.remaining}`}
                  </span>
                )}
                {task.status === 'completed' && <span className="text-green-500">完成</span>}
                {task.status === 'failed' && <span className="text-red-500">失败</span>}
                {task.status === 'pending' && '等待中'}
                {task.status === 'cancelled' && '已取消'}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
